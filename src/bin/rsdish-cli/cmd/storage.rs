use clap::{Args, Subcommand};
use rsdish::phy::stg::{Storage, scan_storages};

#[derive(Debug, Args)]
#[command(about = "Info of storages.")]
pub struct StorageCmd {
    #[command(subcommand)]
    pub subcmd: StorageSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum StorageSubcommand {
    List(StorageListArgs),
}

#[derive(Debug, Args)]
#[command(about = "List all scanned storages.")]
pub struct StorageListArgs {
    #[arg(short, long)]
    pub verbose: bool,
}

pub fn handle_storage(cmd: StorageCmd) {
    match cmd.subcmd {
        StorageSubcommand::List(child) => handle_storage_list(child),
    }
}

pub fn storages() -> Vec<Storage> {
    // every mounted disk plus the custom storages from the user config
    scan_storages().0
}

pub fn handle_storage_list(args: StorageListArgs) {
    let stgs = storages();

    if !args.verbose {
        // TODO: print like storage tree, storage only print storage_abs_path, cabinet only print groupuuids
        for stg in stgs.iter() {
            println!("Storage: \"{}\"", stg.stg_info.abs_path.display());

            let cabs = &stg.cabs;
            for (j, cab) in cabs.iter().enumerate() {
                let cab_prefix = if j == cabs.len() - 1 { "└──" } else { "├──" };
                let cab_rel_path = cab.cab_info.abs_path.strip_prefix(stg.stg_info.abs_path.as_path()).unwrap();
                let group_uuids: Vec<String> = cab
                    .cab_info
                    .cab_conf
                    .memberships
                    .iter()
                    .map(|m| m.group_uuid.clone())
                    .collect();
                println!("{} Cabinet: {:?}, Memberships: {:?}", cab_prefix, cab_rel_path, group_uuids);
            }
        }
        return;
    }

    for stg in stgs {
        println!("{:#?}", stg);
    }
}
