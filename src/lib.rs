use regex::Regex;
use std::{env::current_dir, fs};
use zed_extension_api::{self as zed, GithubRelease};

fn platform() -> zed::Result<(&'static str, &'static str)> {
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
  Ok((os_str, arch_str))
}

struct Mpls {
  language_server_path: Option<String>,
}

impl Mpls {
  // Makes self.language_server_path not None
  fn find_language_server(
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
    );
    if let Ok(release) = release {
      // If we have internet connection
      self.when_online(&release, language_server_id)
    } else {
      // If we don't
      self.when_offline()
    }
  }

  fn when_online(
    &mut self,
    release: &GithubRelease,
    language_server_id: &zed::LanguageServerId,
  ) -> zed::Result<()> {
    let (os, arch) = platform()?;
    let file_type = match os {
      "windows" => zed::DownloadedFileType::Zip,
      "linux" | "darwin" => zed::DownloadedFileType::GzipTar,
      _ => unreachable!("There's a bug in the codebase"),
    };
    let file_type_str = match file_type {
      zed::DownloadedFileType::Zip => "zip",
      zed::DownloadedFileType::GzipTar => "tar.gz",
      zed::DownloadedFileType::Gzip => "gz",
      zed::DownloadedFileType::Uncompressed => "",
    };
    let archived_asset_name = format!(
      "mpls_{}_{}_{}.{}",
      &release.version[1..], // v0.16.0 -> 0.16.0
      os,
      arch,
      file_type_str
    );
    let unarchived_asset_name = format!(
      "mpls_{}_{}_{}",
      &release.version[1..], // v0.16.0 -> 0.16.0
      os,
      arch
    );
    let executable_path = format!("{}/{}", unarchived_asset_name, "mpls");

    if let Ok(true) = fs::exists(&executable_path) {
      // The language server is already downloaded.
      zed::make_file_executable(&executable_path)?;
      self.language_server_path.replace(executable_path);
      return Ok(());
    }

    // If there was an update, we download the new language server.
    let asset = release
      .assets
      .iter()
      .find(|asset| asset.name == archived_asset_name)
      .ok_or(format!("Can't find the executable in MPLS GitHub release."))?;
    zed::set_language_server_installation_status(
      language_server_id,
      &zed::LanguageServerInstallationStatus::Downloading,
    );
    zed::download_file(&asset.download_url, &unarchived_asset_name, file_type)?;

    zed::make_file_executable(&executable_path)?;
    self.language_server_path.replace(executable_path);

    Ok(())
  }

  fn when_offline(&mut self) -> zed::Result<()> {
    let (os, arch) = platform()?;
    let unarchived_asset_pattern = format!(r"^mpls_([0-9]+)\.([0-9]+)\.([0-9]+)_{}_{}$", os, arch);
    let unarchived_asset_regex = Regex::new(&unarchived_asset_pattern).unwrap();

    let mut version_triples: Vec<(usize, usize, usize)> = Vec::new();
    for dir in current_dir()
      .and_then(fs::read_dir)
      .map_err(|err| err.to_string())?
    {
      let dir = dir.map_err(|err| err.to_string())?;
      if !dir.file_type().map_err(|err| err.to_string())?.is_dir() {
        continue;
      }

      let dirname = dir.file_name();
      let dirname = dirname
        .to_str()
        .ok_or("dirname contains invalid UTF-8 string")?;

      if let Some(captures) = unarchived_asset_regex.captures(dirname) {
        // It's safe to unwrap here because [0-9] only captures ASCII digits. parse() never panics.
        version_triples.push((
          captures[1].parse().unwrap(),
          captures[2].parse().unwrap(),
          captures[3].parse().unwrap(),
        ));
      }
    }

    version_triples.sort();
    let latest_installed_version = version_triples.last().ok_or("No installation of MPLS has found. We can't install it because we have no internet connection.")?;
    let executable_path = format!(
      "mpls_{}.{}.{}_{}_{}/mpls",
      latest_installed_version.0, latest_installed_version.1, latest_installed_version.2, os, arch
    );

    zed::make_file_executable(&executable_path)?;
    self.language_server_path.replace(executable_path);

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
    if let Err(err) = self.find_language_server(language_server_id, worktree) {
      zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::Failed(err.to_string()),
      );
      return Err(err);
    } else {
      zed::set_language_server_installation_status(
        language_server_id,
        &zed::LanguageServerInstallationStatus::None,
      );
    }

    Ok(zed::Command::new(self.language_server_path.as_ref().expect("This shouldn't happen. self.install_language_server() is supposed to make self.language_server_path not None")).arg("--enable-emoji").arg("--enable-wikilinks").arg("--enable-footnotes"))
  }
}

zed::register_extension!(Mpls);
