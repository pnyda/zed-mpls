use std::fs;

use zed_extension_api as zed;

struct Mpls {
  language_server_path: Option<String>,
}

impl Mpls {
  fn install_language_server(
    &mut self,
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
  ) -> zed::Result<()> {
    // Do nothing if the language server is already installed.
    if self.language_server_path.is_some() {
      return Ok(());
    }

    if let Some(path) = worktree.which("mpls") {
      self.language_server_path.replace(path);
      return Ok(());
    }

    // Check for updates.
    zed::set_language_server_installation_status(
      language_server_id,
      &zed::LanguageServerInstallationStatus::CheckingForUpdate,
    );
    let release = zed::latest_github_release(
      "mhersson/mpls",
      zed::GithubReleaseOptions {
        require_assets: true,
        pre_release: false,
      },
    )?;

    let (os, arch) = zed::current_platform();
    let os_str = match os {
      zed::Os::Linux => "linux",
      zed::Os::Mac => "darwin",
      zed::Os::Windows => "windows",
    };
    let arch_str = match arch {
      zed::Architecture::Aarch64 => "arm64",
      zed::Architecture::X8664 => "amd64",
      _ => return Err(format!("{:?} is not supported by MPLS", arch)),
    };
    let file_type = match os {
      zed::Os::Windows => zed::DownloadedFileType::Zip,
      zed::Os::Linux | zed::Os::Mac => zed::DownloadedFileType::GzipTar,
    };
    let file_type_str = match file_type {
      zed::DownloadedFileType::Zip => "zip",
      zed::DownloadedFileType::GzipTar => "tar.gz",
      zed::DownloadedFileType::Gzip => "gz",
      zed::DownloadedFileType::Uncompressed => "",
    };
    let archived_asset_name = format!(
      "mpls_{}_{}_{}.{}",
      release.version, os_str, arch_str, file_type_str
    );
    let unarchived_asset_name = format!("mpls_{}_{}_{}", release.version, os_str, arch_str);
    let executable_name = format!("{}/{}", unarchived_asset_name, "mpls");
    if let Ok(true) = fs::exists(&executable_name) {
      // The language server is already downloaded.
      zed::make_file_executable(&executable_name)?;
      self.language_server_path.replace(executable_name);
      return Ok(());
    }

    // If there was an update, we download the new language server.
    zed::set_language_server_installation_status(
      language_server_id,
      &zed::LanguageServerInstallationStatus::Downloading,
    );
    let asset = release
      .assets
      .iter()
      .find(|asset| asset.name == archived_asset_name)
      .ok_or(format!("Can't find the executable in MPLS GitHub release."))?;
    zed::download_file(&asset.download_url, &unarchived_asset_name, file_type)?;
    zed::make_file_executable(&executable_name)?;
    self.language_server_path.replace(executable_name);
    Ok(())
  }
}

impl zed::Extension for Mpls {
  fn new() -> Self {
    Self {
      language_server_path: None,
    }
  }

  fn language_server_command(
    &mut self,
    language_server_id: &zed::LanguageServerId,
    worktree: &zed::Worktree,
  ) -> zed::Result<zed::Command> {
    self.install_language_server(language_server_id, worktree)?;

    Ok(zed::Command::new(self.language_server_path.as_ref().expect("This shouldn't happen. self.install_language_server() is supposed to make self.language_server_path not None")).arg("--enable-emoji").arg("--enable-wikilinks").arg("--enable-footnotes"))
  }
}

zed::register_extension!(Mpls);
