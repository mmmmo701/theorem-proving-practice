//! `vault` command group: list/add/switch vaults and query the current one.
//! Every other command operates on whichever vault this leaves current (or on
//! `--vault NAME` for a single invocation) — see [`App::bootstrap_in`].
//!
//! [`App::bootstrap_in`]: crate::app::App::bootstrap_in

use crate::app::VaultEnv;
use crate::cli::args::{VaultAddArgs, VaultCommand, VaultSwitchArgs};
use crate::cli::{CliError, input};
use crate::domain::VaultName;

pub fn run(vaults: &VaultEnv, command: VaultCommand) -> Result<(), CliError> {
    match command {
        VaultCommand::List => list(vaults),
        VaultCommand::Current => current(vaults),
        VaultCommand::Add(args) => add(vaults, args),
        VaultCommand::Switch(args) => switch(vaults, args),
    }
}

fn list(vaults: &VaultEnv) -> Result<(), CliError> {
    let all = vaults.list_vaults()?;
    if all.is_empty() {
        println!("No vaults yet. Create one with `vault add <name>`.");
        return Ok(());
    }
    let current = vaults.current_vault()?;
    for v in &all {
        let marker = if *v == current { "*" } else { " " };
        println!("{marker} {v}");
    }
    Ok(())
}

fn current(vaults: &VaultEnv) -> Result<(), CliError> {
    println!("{}", vaults.current_vault()?);
    Ok(())
}

fn add(vaults: &VaultEnv, args: VaultAddArgs) -> Result<(), CliError> {
    let name = if args.interactive {
        gather_name_interactively(args.name)?
    } else {
        let raw = args.name.expect("clap requires name unless --interactive");
        VaultName::new(raw).map_err(crate::app::AppError::from)?
    };

    vaults.add_vault(&name)?;
    println!("Created vault '{name}'.");

    if args.interactive && input::confirm(&format!("Switch to '{name}' now?"))? {
        vaults.switch_vault(&name)?;
        println!("Switched to vault '{name}'.");
    }
    Ok(())
}

/// Prompt for a vault name, re-prompting until it validates. Shares the
/// validated re-prompt loop `add -i` / `edit -i` use for theorem fields.
fn gather_name_interactively(prefill: Option<String>) -> Result<VaultName, CliError> {
    let raw = super::prompt_label("Vault name", prefill, |s| VaultName::new(s).map(drop))?;
    Ok(VaultName::new(raw).expect("prompt_label already validated this"))
}

fn switch(vaults: &VaultEnv, args: VaultSwitchArgs) -> Result<(), CliError> {
    let target = if args.interactive {
        select_from_menu(vaults)?
    } else {
        let query = args.name.as_deref().expect("clap requires name unless --interactive");
        Some(resolve_vault_name(vaults, query)?)
    };
    let Some(target) = target else {
        eprintln!("Cancelled; vault unchanged.");
        return Ok(());
    };

    if target == vaults.current_vault()? {
        eprintln!("Already in vault '{target}'.");
        return Ok(());
    }

    vaults.switch_vault(&target)?;
    println!("Switched to vault '{target}'.");
    Ok(())
}

/// Present a numbered menu of all vaults and return the chosen one, or `None`
/// if there are none yet or the user cancels with a blank entry.
fn select_from_menu(vaults: &VaultEnv) -> Result<Option<VaultName>, CliError> {
    let all = vaults.list_vaults()?;
    if all.is_empty() {
        eprintln!("No vaults yet; use 'vault add' to create one.");
        return Ok(None);
    }
    let current = vaults.current_vault()?;

    eprintln!("Vaults:");
    for (i, v) in all.iter().enumerate() {
        let marker = if *v == current { "*" } else { " " };
        eprintln!("{:>3}. {marker} {v}", i + 1);
    }

    input::pick_from_menu(
        "Switch to which? (number or name, blank to cancel)",
        all.len(),
        |n| all.get(n - 1).cloned(),
        |choice| match resolve_vault_name(vaults, choice) {
            Ok(v) => Ok(Some(v)),
            Err(CliError::VaultNotFound { .. }) => {
                eprintln!("  ! no vault matches '{choice}'");
                Ok(None)
            }
            Err(CliError::AmbiguousVault { count, .. }) => {
                eprintln!("  ! '{choice}' matches {count} vaults; use a longer prefix");
                Ok(None)
            }
            Err(other) => Err(other),
        },
    )
}

/// Resolve a query to exactly one vault: an exact name match wins; otherwise
/// a unique prefix match. Zero or many become the standard CLI errors.
fn resolve_vault_name(vaults: &VaultEnv, query: &str) -> Result<VaultName, CliError> {
    let all = vaults.list_vaults()?;
    let needle = query.trim().to_lowercase();

    if let Some(v) = all.iter().find(|v| v.as_str() == needle) {
        return Ok(v.clone());
    }

    let hits: Vec<&VaultName> = all.iter().filter(|v| v.as_str().starts_with(&needle)).collect();
    match hits.as_slice() {
        [] => Err(CliError::VaultNotFound {
            query: query.to_string(),
        }),
        [v] => Ok((*v).clone()),
        many => Err(CliError::AmbiguousVault {
            query: query.to_string(),
            count: many.len(),
        }),
    }
}
