use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use toml::Value;

use crate::nixpacks::plan::{
    phase::{Phase, StartPhase},
    BuildPlan,
};

use super::Provider;

#[derive(Serialize, Deserialize, Debug)]
struct PixiTasks {
    build: Option<Value>,
    start: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PixiToml {
    tasks: PixiTasks,
}

pub struct PixiProvider;

impl Provider for PixiProvider {
    fn name(&self) -> &str {
        "pixi"
    }

    fn detect(
        &self,
        app: &crate::nixpacks::app::App,
        _env: &crate::nixpacks::environment::Environment,
    ) -> anyhow::Result<bool> {
        Ok(app.has_match("pixi.toml"))
    }

    fn get_build_plan(
        &self,
        app: &crate::nixpacks::app::App,
        _environment: &crate::nixpacks::environment::Environment,
    ) -> anyhow::Result<Option<crate::nixpacks::plan::BuildPlan>> {
        let config = app.read_toml::<PixiToml>("pixi.toml")?;
        let mut plan = BuildPlan::default();

        let mut setup = Phase::new("setup");
        setup.only_include_files = Some(vec![]);
        setup.add_cmd("curl -fsSL https://pixi.sh/install.sh | bash");
        plan.add_phase(setup);

    let mut install = Phase::install(Some("~/.pixi/bin/pixi install".to_string()));
        install.only_include_files = Some(vec!["pixi.toml".to_string(), "pixi.lock".to_string()]);
        plan.add_phase(install);

        if config.tasks.build.is_some() {
            plan.add_phase(Phase::build(Some("~/.pixi/bin/pixi run build".to_string())));
        }

        if config.tasks.start.is_some() {
            plan.set_start_phase(StartPhase::new("~/.pixi/bin/pixi run start"));
        } else {
            return Err(anyhow!(
                "No start task provided; please add one to your pixi.toml."
            ));
        }

        Ok(Some(plan))
    }
}
