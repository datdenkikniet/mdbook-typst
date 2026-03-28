use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;
use typst::{diag::PackageError, syntax::package::PackageSpec};

#[derive(Debug, Clone)]
pub enum AutoDownload {
    BuiltIn,
    Custom(String),
}

fn other(value: impl AsRef<str>) -> PackageError {
    PackageError::Other(Some(value.as_ref().into()))
}

impl AutoDownload {
    pub fn download_if_absent(
        &self,
        tar: &Path,
        package: &PackageSpec,
        allowed_licenses: &[String],
    ) -> Result<(), PackageError> {
        if tar.exists() {
            return Ok(());
        }

        match self {
            AutoDownload::BuiltIn => Self::builtin(tar, package, allowed_licenses),
            AutoDownload::Custom(command) => Self::custom(command, tar, package, allowed_licenses),
        }
    }

    fn custom(
        command: &str,
        tar: &Path,
        package: &PackageSpec,
        allowed_licenses: &[String],
    ) -> Result<(), PackageError> {
        let tar = tar.display().to_string();
        let package = package.to_string();

        let result = Command::new(command)
            .args([tar, package])
            .args(allowed_licenses)
            .stderr(Stdio::inherit())
            .output()
            .map_err(|e| {
                other(format!(
                    "Failed to execute custom command '{command}'. Error: {e}"
                ))
            })?;

        if !result.status.success() {
            return Err(other(format!(
                "Custom command '{command}' exited with non-success status code."
            )));
        }

        Ok(())
    }

    fn builtin(
        tar: &Path,
        package: &PackageSpec,
        allowed_licenses: &[String],
    ) -> Result<(), PackageError> {
        let dl_tar = format!(
            "{name}-{version}.tar",
            name = package.name,
            version = package.version,
        );
        let dl_tar_gz = format!("{dl_tar}.gz");
        let to_download = format!("{namespace}/{dl_tar_gz}", namespace = package.namespace);
        let url = format!("https://packages.typst.org/{to_download}");

        eprintln!("Downloading '{url}' to '{}'...", dl_tar_gz);
        let download = Command::new("wget")
            .arg(&url)
            .output()
            .map_err(|e| other(format!("Failed to run download command: {e}")))?;
        if !download.status.success() {
            return Err(other(format!("Downloading '{url}' failed.")));
        }

        let gunzip = Command::new("gunzip")
            .arg(&dl_tar_gz)
            .output()
            .map_err(|e| other(format!("Failed to run gunzip command: {e}")))?;

        if !gunzip.status.success() {
            return Err(other(format!("Decompressing '{dl_tar_gz}' failed.")));
        }

        if let Err(e) = validate_license(&Path::new(&dl_tar), &allowed_licenses) {
            std::fs::remove_file(&dl_tar).map_err(|e| {
                other(&format!(
                    "failed to remove invalidly-licensed package '{dl_tar}'. Error: {e}",
                ))
            })?;

            return Err(other(format!(
                "failed to validate license for downloaded package '{dl_tar}'. Error: {e}"
            )));
        }

        if let Some(parent) = tar.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| other(format!("Failed to create target directories. Error: {e}")))?;
        }

        std::fs::rename(&dl_tar, tar).map_err(|e| {
            other(format!(
                "Failed to move downloaded file {dl_tar} to target {}. Error: {e}",
                tar.display()
            ))
        })?;

        eprintln!(
            "Succesfully downloaded & installed {package} to {}",
            tar.display()
        );

        Ok(())
    }
}

fn validate_license(tar_file: impl AsRef<Path>, allowed_licenses: &[String]) -> anyhow::Result<()> {
    use std::io::Read;

    let file = std::fs::File::open(&tar_file).context("Failed to open downloaded package")?;
    let mut archive = tar::Archive::new(file);

    for entry in archive
        .entries()
        .context("Downloaded tar file was malformed")?
    {
        let mut entry = entry.context("getting entry")?;
        let path = entry.path().context("getting path")?.to_path_buf();

        if path.as_path() == "typst.toml" {
            let mut output = String::new();
            entry
                .read_to_string(&mut output)
                .context("Failed to read typst.toml")?;

            let toml =
                toml::from_str::<toml::Table>(&output).context("typst.toml was not valid toml")?;

            let package = toml
                .get("package")
                .map(|v| v.as_table())
                .flatten()
                .context("typst.toml has no package table")?;

            let license = package
                .get("license")
                .map(|v| v.as_str())
                .flatten()
                .context("package defines no license")?
                .to_string();

            if allowed_licenses.contains(&license) {
                return Ok(());
            } else {
                anyhow::bail!("License '{license}' is not allowed.")
            }
        }
    }

    anyhow::bail!("Could not find `typst.toml` in downloaded package");
}
