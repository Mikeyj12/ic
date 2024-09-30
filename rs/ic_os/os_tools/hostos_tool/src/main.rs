use std::path::Path;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use config::config_ini::get_config_ini_settings;
use config::deployment_json::get_deployment_settings;
use config::types::NetworkSettings;
use config::{DEFAULT_HOSTOS_CONFIG_INI_FILE_PATH, DEFAULT_HOSTOS_DEPLOYMENT_JSON_PATH};
use network::generate_network_config;
use network::ipv6::generate_ipv6_address;
use network::mac_address::{generate_mac_address, get_ipmi_mac, FormattedMacAddress};
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
        #[arg(short, long, default_value = "HostOS")]
        node_type: String,
    },
    GenerateIpv6Address {
        #[arg(short, long, default_value = "HostOS")]
        node_type: String,
    },
}

#[derive(Parser)]
struct HostOSArgs {
    #[arg(short, long, default_value_t = DEFAULT_HOSTOS_CONFIG_INI_FILE_PATH.to_string(), value_name = "FILE")]
    config: String,

    #[arg(short, long, default_value_t = DEFAULT_HOSTOS_DEPLOYMENT_JSON_PATH.to_string(), value_name = "FILE")]
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

    let opts = HostOSArgs::parse();

    match opts.command {
        Some(Commands::GenerateNetworkConfig { output_directory }) => {
            let config_ini_settings = get_config_ini_settings(Path::new(&opts.config))?;

            let deployment_json_settings =
                get_deployment_settings(Path::new(&opts.deployment_file))?;
            eprintln!("Deployment config: {:?}", deployment_json_settings);

            // TODO: NODE-1466: Remove in configuration revamp (HostOS and GuestOS integration).
            // Once HostOS is using the config struct, all config will be contained there
            // and we won't need to read config.ini and deployment.json directly.
            let network_settings = NetworkSettings {
                ipv6_prefix: config_ini_settings.ipv6_prefix,
                ipv6_prefix_length: config_ini_settings.ipv6_prefix_length,
                ipv6_gateway: config_ini_settings.ipv6_gateway,
                ipv4_address: config_ini_settings.ipv4_address,
                ipv4_gateway: config_ini_settings.ipv4_gateway,
                ipv4_prefix_length: config_ini_settings.ipv4_prefix_length,
                domain: config_ini_settings.domain,
                mgmt_mac: deployment_json_settings.deployment.mgmt_mac,
            };
            eprintln!("Network settings config: {:?}", &network_settings);

            let mgmt_mac = match network_settings.mgmt_mac.as_ref() {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_json_settings.deployment.name.as_str(),
                &NodeType::HostOS,
            )?;
            eprintln!("Using generated mac (unformatted) {}", generated_mac);

            generate_network_config(
                &network_settings,
                generated_mac,
                Path::new(&output_directory),
            )
        }
        Some(Commands::GenerateIpv6Address { node_type }) => {
            let config_ini_settings = get_config_ini_settings(Path::new(&opts.config))?;

            let deployment_json_settings =
                get_deployment_settings(Path::new(&opts.deployment_file))?;
            eprintln!("Deployment config: {:?}", deployment_json_settings);

            // TODO: NODE-1466: Remove in configuration revamp (HostOS and GuestOS integration).
            // Once HostOS is using the config struct, all config will be contained there
            // and we won't need to read config.ini and deployment.json directly.
            let network_settings = NetworkSettings {
                ipv6_prefix: config_ini_settings.ipv6_prefix,
                ipv6_prefix_length: config_ini_settings.ipv6_prefix_length,
                ipv6_gateway: config_ini_settings.ipv6_gateway,
                ipv4_address: config_ini_settings.ipv4_address,
                ipv4_gateway: config_ini_settings.ipv4_gateway,
                ipv4_prefix_length: config_ini_settings.ipv4_prefix_length,
                domain: config_ini_settings.domain,
                mgmt_mac: deployment_json_settings.deployment.mgmt_mac,
            };
            eprintln!("Network settings config: {:?}", &network_settings);

            let mgmt_mac = match network_settings.mgmt_mac.as_ref() {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_json_settings.deployment.name.as_str(),
                &node_type,
            )?;
            eprintln!("Using generated mac (unformatted) {}", generated_mac);
            let ipv6_address =
                generate_ipv6_address(&network_settings.ipv6_prefix, &generated_mac)?;
            println!(
                "{}",
                to_cidr(ipv6_address, network_settings.ipv6_prefix_length)
            );
            Ok(())
        }
        Some(Commands::GenerateMacAddress { node_type }) => {
            let config_ini_settings = get_config_ini_settings(Path::new(&opts.config))?;

            let deployment_json_settings =
                get_deployment_settings(Path::new(&opts.deployment_file))?;
            eprintln!("Deployment config: {:?}", deployment_json_settings);

            // TODO: NODE-1466: Remove in configuration revamp (HostOS and GuestOS integration).
            // Once HostOS is using the config struct, all config will be contained there
            // and we won't need to read config.ini and deployment.json directly.
            let network_settings = NetworkSettings {
                ipv6_prefix: config_ini_settings.ipv6_prefix,
                ipv6_prefix_length: config_ini_settings.ipv6_prefix_length,
                ipv6_gateway: config_ini_settings.ipv6_gateway,
                ipv4_address: config_ini_settings.ipv4_address,
                ipv4_gateway: config_ini_settings.ipv4_gateway,
                ipv4_prefix_length: config_ini_settings.ipv4_prefix_length,
                domain: config_ini_settings.domain,
                mgmt_mac: deployment_json_settings.deployment.mgmt_mac,
            };
            eprintln!("Network settings config: {:?}", &network_settings);

            let node_type = node_type.parse::<NodeType>()?;

            let mgmt_mac = match network_settings.mgmt_mac.as_ref() {
                Some(config_mac) => {
                    let mgmt_mac = FormattedMacAddress::try_from(config_mac.as_str())?;
                    eprintln!(
                        "Using mgmt_mac address found in deployment.json: {}",
                        mgmt_mac
                    );
                    mgmt_mac
                }
                None => get_ipmi_mac()?,
            };
            let generated_mac = generate_mac_address(
                &mgmt_mac,
                deployment_json_settings.deployment.name.as_str(),
                &node_type,
            )?;

            println!("{}", generated_mac);
            Ok(())
        }
        None => Err(anyhow!(
            "No subcommand specified. Run with '--help' for subcommands"
        )),
    }
}
