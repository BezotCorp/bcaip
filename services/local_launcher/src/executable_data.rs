use crate::executable::Executable;
use bincode_next::decode_from_slice;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(crate) fn load_definitions(
    data_directory: &Path,
) -> Result<Vec<Executable>, String> {
    let local =
        data_directory.join(
            "executables.local.ron",
        );

    let production =
        data_directory.join(
            "executables.ron",
        );

    let path = if local.exists() {
        local
    } else {
        production
    };

    let source =
        fs::read_to_string(&path)
            .map_err(|error| {
                format!(
                    "failed to read {}: {error}",
                    path.display()
                )
            })?;

    let executables: Vec<Executable> =
        ron::from_str(&source).map_err(
            |error| {
                format!(
                    "failed to decode {}: {error}",
                    path.display()
                )
            },
        )?;

    validate(&executables)?;

    Ok(executables)
}

pub(crate) fn load_runtime(
    data_directory: &Path,
) -> Result<Vec<Executable>, String> {
    let path =
        data_directory.join(
            "executables.bin",
        );

    let bytes =
        fs::read(&path).map_err(
            |error| {
                format!(
                    "failed to read {}: {error}",
                    path.display()
                )
            },
        )?;

    let config =
        bincode_next::config::standard();

    let (executables, consumed) =
        decode_from_slice::<
            Vec<Executable>,
            _,
        >(
            &bytes,
            config,
        )
        .map_err(|error| {
            format!(
                "failed to decode {}: {error}",
                path.display()
            )
        })?;

    if consumed != bytes.len() {
        return Err(format!(
            "{} contains trailing bytes",
            path.display()
        ));
    }

    Ok(executables)
}

fn validate(
    executables: &[Executable],
) -> Result<(), String> {
    let mut names =
        HashSet::new();

    let mut addresses =
        HashSet::new();

    for executable in executables {
        executable
            .validate_definition()?;

        if !names.insert(
            executable.name.as_str(),
        ) {
            return Err(format!(
                "duplicate executable: {}",
                executable.name
            ));
        }

        if !addresses.insert(
            executable.default_ws_address,
        ) {
            return Err(format!(
                "duplicate default WebSocket address: {}",
                executable.default_ws_address
            ));
        }
    }

    Ok(())
}
