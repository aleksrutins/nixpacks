use std::collections::HashMap;

use super::Provider;
use crate::nixpacks::{
    app::App,
    environment::{Environment, EnvironmentVariables},
    nix::{NixConfig, Pkg},
};
use anyhow::{bail, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub struct NpmProvider {}

const AVAILABLE_NODE_VERSIONS: &[u32] = &[10, 12, 14, 16, 17];
const DEFAULT_NODE_PKG_NAME: &'static &str = &"pkgs.nodejs";

impl Provider for NpmProvider {
    fn name(&self) -> &str {
        "node"
    }

    fn detect(&self, app: &App, _env: &Environment) -> Result<bool> {
        Ok(app.includes_file("package.json"))
    }

    fn pkgs(&self, app: &App, _env: &Environment) -> Result<NixConfig> {
        let package_json: PackageJson = app.read_json("package.json")?;
        let node_pkg = NpmProvider::get_nix_node_pkg(&package_json)?;

        Ok(NixConfig::new(vec![Pkg::new("pkgs.stdenv"), node_pkg]))
    }

    fn install_cmd(&self, _app: &App, _env: &Environment) -> Result<Option<String>> {
        Ok(Some("npm install".to_string()))
    }

    fn suggested_build_cmd(&self, app: &App, _env: &Environment) -> Result<Option<String>> {
        let package_json: PackageJson = app.read_json("package.json")?;
        if let Some(scripts) = package_json.scripts {
            if scripts.get("build").is_some() {
                return Ok(Some("npm run build".to_string()));
            }
        }

        Ok(None)
    }

    fn suggested_start_command(&self, app: &App, _env: &Environment) -> Result<Option<String>> {
        let package_json: PackageJson = app.read_json("package.json")?;
        if let Some(scripts) = package_json.scripts {
            if scripts.get("start").is_some() {
                return Ok(Some("npm run start".to_string()));
            }
        }

        if let Some(main) = package_json.main {
            if app.includes_file(&main) {
                return Ok(Some(format!("node {}", main)));
            }
        }
        if app.includes_file("index.js") {
            return Ok(Some(String::from("node index.js")));
        }

        Ok(None)
    }

    fn get_environment_variables(
        &self,
        _app: &App,
        _env: &Environment,
    ) -> Result<EnvironmentVariables> {
        Ok(NpmProvider::get_node_environment_variables())
    }
}

impl NpmProvider {
    pub fn get_node_environment_variables() -> EnvironmentVariables {
        EnvironmentVariables::from([
            ("NODE_ENV".to_string(), "production".to_string()),
            ("NPM_CONFIG_PRODUCTION".to_string(), "false".to_string()),
        ])
    }

    /// Parses the package.json engines field and returns a Nix package if available
    pub fn get_nix_node_pkg(package_json: &PackageJson) -> Result<Pkg> {
        let node_version = package_json
            .engines
            .as_ref()
            .and_then(|engines| engines.get("node"));

        let node_version = match node_version {
            Some(node_version) => node_version,
            None => return Ok(Pkg::new(DEFAULT_NODE_PKG_NAME)),
        };

        // Any version will work, use latest
        if node_version == "*" {
            return Ok(Pkg::new(DEFAULT_NODE_PKG_NAME));
        }

        // Parse `12` or `12.x` into nodejs-12_x
        let re = Regex::new(r"^(\d+)\.?x?$").unwrap();
        if let Some(node_pkg) = parse_regex_into_pkg(&re, node_version)? {
            return Ok(Pkg::new(node_pkg.as_str()));
        }

        // Parse `>=14.10.3 <16` into nodejs-14_x
        let re = Regex::new(r"^>=(\d+)").unwrap();
        if let Some(node_pkg) = parse_regex_into_pkg(&re, node_version)? {
            return Ok(Pkg::new(node_pkg.as_str()));
        }

        Ok(Pkg::new(DEFAULT_NODE_PKG_NAME))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PackageJson {
    pub name: String,
    pub scripts: Option<HashMap<String, String>>,
    pub engines: Option<HashMap<String, String>>,
    pub main: Option<String>,
}

fn version_number_to_pkg(version: &u32) -> Result<Option<String>> {
    if AVAILABLE_NODE_VERSIONS.contains(version) {
        Ok(Some(format!("nodejs-{}_x", version)))
    } else {
        bail!("Node version {} is not available", version);
    }
}

fn parse_regex_into_pkg(re: &Regex, node_version: &str) -> Result<Option<String>> {
    let matches: Vec<_> = re.captures_iter(node_version).collect();
    if let Some(m) = matches.get(0) {
        match m[1].parse::<u32>() {
            Ok(version) => return version_number_to_pkg(&version),
            Err(_e) => {}
        }
    }

    Ok(None)
}

#[cfg(test)]
mod test {
    use super::*;

    fn engines_node(version: &str) -> Option<HashMap<String, String>> {
        Some(HashMap::from([("node".to_string(), version.to_string())]))
    }

    #[test]
    fn test_no_engines() -> Result<()> {
        assert_eq!(
            NpmProvider::get_nix_node_pkg(&PackageJson {
                name: String::default(),
                main: None,
                scripts: None,
                engines: None
            })?,
            Pkg::new(DEFAULT_NODE_PKG_NAME)
        );

        Ok(())
    }

    #[test]
    fn test_star_engine() -> Result<()> {
        assert_eq!(
            NpmProvider::get_nix_node_pkg(&PackageJson {
                name: String::default(),
                main: None,
                scripts: None,
                engines: engines_node("*")
            })?,
            Pkg::new(DEFAULT_NODE_PKG_NAME)
        );

        Ok(())
    }

    #[test]
    fn test_simple_engine() -> Result<()> {
        assert_eq!(
            NpmProvider::get_nix_node_pkg(&PackageJson {
                name: String::default(),
                main: None,
                scripts: None,
                engines: engines_node("14"),
            })?,
            Pkg::new("nodejs-14_x")
        );

        Ok(())
    }

    #[test]
    fn test_simple_engine_x() -> Result<()> {
        assert_eq!(
            NpmProvider::get_nix_node_pkg(&PackageJson {
                name: String::default(),
                main: None,
                scripts: None,
                engines: engines_node("12.x"),
            })?,
            Pkg::new("nodejs-12_x")
        );

        Ok(())
    }

    #[test]
    fn test_engine_range() -> Result<()> {
        assert_eq!(
            NpmProvider::get_nix_node_pkg(&PackageJson {
                name: String::default(),
                main: None,
                scripts: None,
                engines: engines_node(">=14.10.3 <16"),
            })?,
            Pkg::new("nodejs-14_x")
        );

        Ok(())
    }

    #[test]
    fn test_engine_invalid_version() -> Result<()> {
        assert!(NpmProvider::get_nix_node_pkg(&PackageJson {
            name: String::default(),
            main: None,
            scripts: None,
            engines: engines_node("15"),
        })
        .is_err());

        Ok(())
    }
}
