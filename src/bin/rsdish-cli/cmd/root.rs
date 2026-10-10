use clap::{Parser, Subcommand};

use clap::builder::{
    Styles,
    styling::{AnsiColor, Effects},
};

use crate::cmd::cabinet::{CabinetCmd, handle_cabinet};
use crate::cmd::config::{ConfigCmd, handle_config};
use crate::cmd::group::{GroupCmd, handle_group};
use crate::cmd::preset::{PresetCmd, handle_preset};
use crate::cmd::storage::{StorageCmd, handle_storage};
use crate::cmd::web::{WebCmd, handle_web};

// Configures Clap v3-style help menu colors
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser, Debug)]
#[command(name = "rsdish")]
#[command(about = "A multi-functional sync tool for domestic storages.", long_about = None)]
#[command(styles=STYLES)]
pub struct RootCmd {
    #[command(subcommand)]
    pub subcmd: SubcommandEnum,
}

#[derive(Subcommand, Debug)]
pub enum SubcommandEnum {
    #[command(visible_alias = "cab")]
    Cabinet(CabinetCmd),
    #[command(visible_alias = "stg")]
    Storage(StorageCmd),
    #[command(visible_alias = "gp")]
    Group(GroupCmd),
    Config(ConfigCmd),
    Preset(PresetCmd),
    Web(WebCmd),
}

pub fn handle_root(cmd: RootCmd) {
    match cmd.subcmd {
        SubcommandEnum::Cabinet(child) => handle_cabinet(child),
        SubcommandEnum::Storage(child) => handle_storage(child),
        SubcommandEnum::Group(child) => handle_group(child),
        SubcommandEnum::Config(child) => handle_config(child),
        SubcommandEnum::Preset(child) => handle_preset(child),
        SubcommandEnum::Web(child) => handle_web(child),
    }
}
