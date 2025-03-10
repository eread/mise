use std::path::{Path, PathBuf};

use crate::env::PATH_KEY;
use crate::file::touch_dir;
use crate::path_env::PathEnv;
use crate::shell::{ActivateOptions, Shell, ShellType, get_shell};
use crate::{dirs, env};
use eyre::Result;
use itertools::Itertools;

/// Initializes mise in the current shell session
///
/// This should go into your shell's rc file or login shell.
/// Otherwise, it will only take effect in the current session.
/// (e.g. ~/.zshrc, ~/.zprofile, ~/.zshenv, ~/.bashrc, ~/.bash_profile, ~/.profile, ~/.config/fish/config.fish, or $PROFILE for powershell)
///
/// Typically, this can be added with something like the following:
///
///     echo 'eval "$(mise activate zsh)"' >> ~/.zshrc
///
/// However, this requires that "mise" is in your PATH. If it is not, you need to
/// specify the full path like this:
///
///     echo 'eval "$(/path/to/mise activate zsh)"' >> ~/.zshrc
///
/// Customize status output with `status` settings.
#[derive(Debug, clap::Args)]
#[clap(verbatim_doc_comment, after_long_help = AFTER_LONG_HELP)]
pub struct Activate {
    /// Shell type to generate the script for
    #[clap(long, short, hide = true)]
    shell: Option<ShellType>,

    /// Shell type to generate the script for
    #[clap()]
    shell_type: Option<ShellType>,

    /// Show "mise: <PLUGIN>@<VERSION>" message when changing directories
    #[clap(long, hide = true)]
    status: bool,

    /// Use shims instead of modifying PATH
    /// Effectively the same as:
    ///
    ///     PATH="$HOME/.local/share/mise/shims:$PATH"
    ///
    /// `mise activate --shims` does not support all the features of `mise activate`.
    /// See https://mise.jdx.dev/dev-tools/shims.html#shims-vs-path for more information
    #[clap(long, verbatim_doc_comment)]
    shims: bool,

    /// Suppress non-error messages
    #[clap(long, short)]
    quiet: bool,

    /// Do not automatically call hook-env
    ///
    /// This can be helpful for debugging mise. If you run `eval "$(mise activate --no-hook-env)"`, then
    /// you can call `mise hook-env` manually which will output the env vars to stdout without actually
    /// modifying the environment. That way you can do things like `mise hook-env --trace` to get more
    /// information or just see the values that hook-env is outputting.
    #[clap(long)]
    no_hook_env: bool,
}

impl Activate {
    pub fn run(self) -> Result<()> {
        let shell = get_shell(self.shell_type.or(self.shell))
            .expect("no shell provided. Run `mise activate zsh` or similar");

        // touch ROOT to allow hook-env to run
        let _ = touch_dir(&dirs::DATA);

        let mise_bin = if cfg!(target_os = "linux") {
            // linux dereferences symlinks, so use argv0 instead
            PathBuf::from(&*env::ARGV0)
        } else {
            env::MISE_BIN.clone()
        };
        match self.shims {
            true => self.activate_shims(shell.as_ref(), &mise_bin)?,
            false => self.activate(shell.as_ref(), &mise_bin)?,
        }

        Ok(())
    }

    fn activate_shims(&self, shell: &dyn Shell, mise_bin: &Path) -> std::io::Result<()> {
        let exe_dir = mise_bin.parent().unwrap();
        miseprint!("{}", self.prepend_path(shell, exe_dir))?;
        miseprint!("{}", self.prepend_path(shell, &dirs::SHIMS))?;
        Ok(())
    }

    fn activate(&self, shell: &dyn Shell, mise_bin: &Path) -> std::io::Result<()> {
        remove_shims(shell)?;
        let exe_dir = mise_bin.parent().unwrap();
        let mut flags = vec![];
        if self.quiet {
            flags.push(" --quiet");
        }
        if self.status {
            flags.push(" --status");
        }
        miseprint!("{}", self.prepend_path(shell, exe_dir))?;
        miseprint!(
            "{}",
            shell.activate(ActivateOptions {
                exe: mise_bin.to_path_buf(),
                flags: flags.join(""),
                no_hook_env: self.no_hook_env,
            })
        )?;
        Ok(())
    }

    fn prepend_path(&self, shell: &dyn Shell, p: &Path) -> String {
        if is_dir_not_in_nix(p) && !is_dir_in_path(p) && !p.is_relative() {
            shell.prepend_env(&PATH_KEY, p.to_string_lossy().as_ref())
        } else {
            String::new()
        }
    }
}

fn remove_shims(shell: &dyn Shell) -> std::io::Result<()> {
    let shims = dirs::SHIMS
        .canonicalize()
        .unwrap_or(dirs::SHIMS.to_path_buf());
    if env::PATH
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .contains(&shims)
    {
        let path_env = PathEnv::from_iter(env::PATH.clone());
        // PathEnv automatically removes the shims directory
        let cmd = shell.set_env(&PATH_KEY, path_env.join().to_string_lossy().as_ref());
        miseprintln!("{cmd}");
    }
    Ok(())
}

fn is_dir_in_path(dir: &Path) -> bool {
    let dir = dir.canonicalize().unwrap_or(dir.to_path_buf());
    env::PATH
        .clone()
        .into_iter()
        .any(|p| p.canonicalize().unwrap_or(p) == dir)
}

fn is_dir_not_in_nix(dir: &Path) -> bool {
    !dir.canonicalize()
        .unwrap_or(dir.to_path_buf())
        .starts_with("/nix/")
}

static AFTER_LONG_HELP: &str = color_print::cstr!(
    r#"<bold><underline>Examples:</underline></bold>

    $ <bold>eval "$(mise activate bash)"</bold>
    $ <bold>eval "$(mise activate zsh)"</bold>
    $ <bold>mise activate fish | source</bold>
    $ <bold>execx($(mise activate xonsh))</bold>
    $ <bold>(&mise activate pwsh) | Out-String | Invoke-Expression</bold>
"#
);
