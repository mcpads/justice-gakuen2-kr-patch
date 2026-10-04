//! Collect the complete shared MENU population without rendering or emitting
//! a disc. This is also the production entry point before component workers.
use super::{MenuAtlasPlan, MenuAtlasRequest, MenuAtlasReservation};
use crate::development_build_spec::LoadedDevelopmentBuildSpec;
use crate::source_disc::SupportedSourceDisc;
use anyhow::Result;

fn collect(
    spec: &LoadedDevelopmentBuildSpec,
    source: &SupportedSourceDisc,
) -> Result<(Vec<MenuAtlasRequest>, Vec<MenuAtlasReservation>)> {
    let mut requests =
        crate::options::collect_menu_requests(&spec.assets.options, &spec.fonts.options, source)?
            .requests();
    requests.extend(crate::title_menu::collect_menu_requests(
        &spec.assets.title_menu,
        source,
    )?);
    requests.extend(crate::title_notice::collect_menu_requests(
        &spec.assets.title_notice,
        source,
    )?);
    requests.extend(crate::title_overlay_runtime::continue_names::collect_menu_requests());
    let (edit, reservations) =
        crate::edit_runtime_text::collect_menu_requests(&spec.assets.mode_descendants, source)?;
    requests.extend(edit);
    Ok((requests, reservations))
}

pub(crate) fn prepare_development_plan(
    spec: &LoadedDevelopmentBuildSpec,
    source: &SupportedSourceDisc,
) -> Result<MenuAtlasPlan> {
    let (requests, reservations) = collect(spec, source)?;
    MenuAtlasPlan::build(&requests, &reservations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    #[ignore = "requires the private original disc and selected font inputs; emits no ROM"]
    fn current_source_population_is_planned_together_without_cross_context_reallocation() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let spec = crate::development_build_spec::load_development_build_spec(
            &root.join("assets/build/development.json"),
        )
        .unwrap();
        let source = SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (requests, reservations) = collect(&spec, &source).unwrap();
        let plan = prepare_development_plan(&spec, &source).unwrap();
        let without_notices = requests
            .iter()
            .filter(|r| !r.key.starts_with("title_notice:"))
            .cloned()
            .collect::<Vec<_>>();
        let baseline = MenuAtlasPlan::build(&without_notices, &reservations).unwrap();
        for request in &without_notices {
            if !request.contexts.contains("mgtit") {
                assert_eq!(
                    plan.glyph(&request.key).unwrap().code,
                    baseline.glyph(&request.key).unwrap().code,
                    "{} moved",
                    request.key
                );
            }
        }
        crate::options::collect_menu_requests(&spec.assets.options, &spec.fonts.options, &source)
            .unwrap()
            .resolve(&plan)
            .unwrap();
        crate::edit_runtime_text::validate_menu_plan(&spec.assets.mode_descendants, &source, &plan)
            .unwrap();
        eprintln!(
            "planned {} requests against {} source reservations",
            requests.len(),
            reservations.len()
        );
    }
}
