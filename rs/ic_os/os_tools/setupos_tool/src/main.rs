use std::path::Path;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use config::types::SetupOSConfig;
use config::{
    deserialize_config, DEFAULT_SETUPOS_CONFIG_INI_FILE_PATH, DEFAULT_SETUPOS_CONFIG_OBJECT_PATH,
    DEFAULT_SETUPOS_DEPLOYMENT_JSON_PATH,
};
use network::generate_network_config;
use network::ipv6::generate_ipv6_address;
use network::mac_address::{generate_mac_address, FormattedMacAddress};
use network::node_type::NodeType;
use network::systemd::DEFAULT_SYSTEMD_NETWORK_DIR;
use utils::to_cidr;

#[derive(Subcommand)]
pub enum Commands {
    /// Generate systemd network configuration files. Bridges available NIC's for IC IPv6 connectivity.
    GenerateNetworkConfig {
        #[arg(short, long, default_value_t = DEFAULT_SYSTEMD_NETWORK_DIR.to_string(), value_name = "DIR")]
        /// systemd-networkd output directory
        output_directory: String,
    },
    GenerateMacAddress {
        #[arg(short, long, default_value = "SetupOS")]
        node_type: String,
    },
    GenerateIpv6Address {
        #[arg(short, long, default_value = "SetupOS")]
        node_type: String,
    },
}

#[derive(Parser)]
struct SetupOSArgs {
    #[arg(short, long, default_value_t = DEFAULT_SETUPOS_CONFIG_INI_FILE_PATH.to_string(), value_name = "FILE")]
    config: String,

    #[arg(short, long, default_value_t = DEFAULT_SETUPOS_DEPLOYMENT_JSON_PATH.to_string(), value_name = "FILE")]
    /// deployment.json file path
    deployment_file: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

pub fn main() -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("ERROR: this only runs on Linux.");
        std::process::exit(1);
    }
    let opts = SetupOSArgs::parse();

    match opts.command {
        Some(Commands::GenerateNetworkConfig { output_directory }) => {
            let setup_config: SetupOSConfig =
                deserialize_config(DEFAULT_SETUPOS_CONFIG_OBJECT_PATH)?;

            eprintln!(
                "Network settings config: {:?}",
                &setup_config.network_settings
            );

            generate_network_config(
                &setup_config.network_settings,
                &setup_config.icos_settings.hostname,
                NodeType::SetupOS,
                Path::new(&output_directory),
            )
        }
        Some(Commands::GenerateIpv6Address { node_type }) => {
            let setup_config: SetupOSConfig =
                deserialize_config(DEFAULT_SETUPOS_CONFIG_OBJECT_PATH)?;

            let node_type = node_type.parse::<NodeType>()?;

            let mac = generate_mac_address(
                &setup_config.icos_settings.hostname,
                &node_type,
                setup_config.network_settings.mgmt_mac.as_deref(),
            )?;
            let ipv6_prefix = setup_config.network_settings.ipv6_prefix.ok_or_else(|| {
                anyhow!("ipv6_prefix required in config to generate ipv6 address")
            })?;
            let ipv6_address = generate_ipv6_address(&ipv6_prefix, &mac)?;
            println!(
                "{}",
                to_cidr(
                    ipv6_address,
                    setup_config.network_settings.ipv6_prefix_length
                )
            );

            Ok(())
        }
        Some(Commands::GenerateMacAddress { node_type }) => {
            let setup_config: SetupOSConfig =
                deserialize_config(DEFAULT_SETUPOS_CONFIG_OBJECT_PATH)?;

            eprintln!(
                "Network settings config: {:?}",
                &setup_config.network_settings
            );

            let node_type = node_type.parse::<NodeType>()?;

            let mac = generate_mac_address(
                &setup_config.icos_settings.hostname,
                &node_type,
                setup_config.network_settings.mgmt_mac.as_deref(),
            )?;

            let mac = FormattedMacAddress::from(&mac);
            println!("{}", mac.get());
            Ok(())
        }
        None => Err(anyhow!(
            "No subcommand specified. Run with '--help' for subcommands"
        )),
    }
}
