use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::components_parser::{get_icos_manifest, IcosManifest, COMPONENTS_PATH};

pub fn check_unused_components(repo_root: &PathBuf) -> Result<()> {
    let icos_components = get_icos_manifest(repo_root)?;

    let components_path = repo_root.join(COMPONENTS_PATH);

    let mut component_repo_files = HashSet::new();
    collect_component_repo_files(&components_path, &mut component_repo_files)?;

    let mut manifest_sources = HashSet::new();
    collect_manifest_sources(&icos_components, &mut manifest_sources);

    let unused_components: Vec<&PathBuf> =
        component_repo_files.difference(&manifest_sources).collect();

    if !unused_components.is_empty() {
        println!("Unused components:");
        for component in unused_components {
            println!("{}", component.display());
        }
    } else {
        println!("No unused components found.");
    }

    Ok(())
}

fn collect_component_repo_files(dir: &Path, files: &mut HashSet<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_component_repo_files(&path, files)?;
        } else {
            files.insert(path);
        }
    }
    Ok(())
}

fn collect_manifest_sources(icos_manifest: &IcosManifest, manifest_sources: &mut HashSet<PathBuf>) {
    for entry in &icos_manifest.guestos.manifest {
        manifest_sources.insert(entry.source.clone());
    }
    for entry in &icos_manifest.hostos.manifest {
        manifest_sources.insert(entry.source.clone());
    }
    for entry in &icos_manifest.setupos.manifest {
        manifest_sources.insert(entry.source.clone());
    }
    for entry in &icos_manifest.boundary_guestos.manifest {
        manifest_sources.insert(entry.source.clone());
    }
}
