"""
Enumerate every rootfs file dependency for GuestOS
"""

rootfs_files = {
    Label("dev-certs/canister_http_test_ca.cert"): "/dev-certs/canister_http_test_ca.cert",
    Label("etc/chrony/chrony.conf"): "/etc/chrony/chrony.conf",
    Label("etc/crypttab"): "/etc/crypttab",
    Label("etc/default/locale"): "/etc/default/locale",
    Label("etc/default/node_exporter"): "/etc/default/node_exporter",
    Label("etc/filebeat/filebeat.yml.template"): "/etc/filebeat/filebeat.yml.template",
    Label("etc/fstab"): "/etc/fstab",
    Label("etc/hostname"): "/etc/hostname",
    Label("etc/hosts"): "/etc/hosts",
    Label("etc/initramfs-tools/hooks/veritysetup"): "/etc/initramfs-tools/hooks/veritysetup",
    Label("etc/initramfs-tools/initramfs.conf"): "/etc/initramfs-tools/initramfs.conf",
    Label("etc/initramfs-tools/modules"): "/etc/initramfs-tools/modules",
    Label("etc/initramfs-tools/scripts/init-bottom/set-machine-id"): "/etc/initramfs-tools/scripts/init-bottom/set-machine-id",
    Label("etc/initramfs-tools/scripts/init-premount/verity-root"): "/etc/initramfs-tools/scripts/init-premount/verity-root",
    Label("etc/metrics-proxy.yaml"): "/etc/metrics-proxy.yaml",
    Label("etc/nftables.conf"): "/etc/nftables.conf",
    Label("etc/node_exporter/node_exporter.crt"): "/etc/node_exporter/node_exporter.crt",
    Label("etc/node_exporter/node_exporter.key"): "/etc/node_exporter/node_exporter.key",
    Label("etc/node_exporter/web.yml"): "/etc/node_exporter/web.yml",
    Label("etc/resolv.conf"): "/etc/resolv.conf",
    Label("etc/sudoers"): "/etc/sudoers",
    Label("etc/sysctl.d/dfn-max-map-count.conf"): "/etc/sysctl.d/dfn-max-map-count.conf",
    Label("etc/sysctl.d/network-tweaks.conf"): "/etc/sysctl.d/network-tweaks.conf",
    Label("etc/sysctl.d/privileged-ports.conf"): "/etc/sysctl.d/privileged-ports.conf",
    Label("etc/systemd/resolved.conf.d/fallback.conf"): "/etc/systemd/resolved.conf.d/fallback.conf",
    Label("etc/systemd/system-generators/mount-generator"): "/etc/systemd/system-generators/mount-generator",
    Label("etc/systemd/system-generators/systemd-gpt-auto-generator"): "/etc/systemd/system-generators/systemd-gpt-auto-generator",
    Label("etc/systemd/system/bootstrap-ic-node.service"): "/etc/systemd/system/bootstrap-ic-node.service",
    Label("etc/systemd/system/filebeat.service"): "/etc/systemd/system/filebeat.service",
    Label("etc/systemd/system/fstrim_tool.service"): "/etc/systemd/system/fstrim_tool.service",
    Label("etc/systemd/system/fstrim_tool.timer"): "/etc/systemd/system/fstrim_tool.timer",
    Label("etc/systemd/system/generate-network-config.service"): "/etc/systemd/system/generate-network-config.service",
    Label("etc/systemd/system/ic-btc-mainnet-adapter.service"): "/etc/systemd/system/ic-btc-mainnet-adapter.service",
    Label("etc/systemd/system/ic-btc-mainnet-adapter.socket"): "/etc/systemd/system/ic-btc-mainnet-adapter.socket",
    Label("etc/systemd/system/ic-btc-testnet-adapter.service"): "/etc/systemd/system/ic-btc-testnet-adapter.service",
    Label("etc/systemd/system/ic-btc-testnet-adapter.socket"): "/etc/systemd/system/ic-btc-testnet-adapter.socket",
    Label("etc/systemd/system/ic-crypto-csp.service"): "/etc/systemd/system/ic-crypto-csp.service",
    Label("etc/systemd/system/ic-crypto-csp.socket"): "/etc/systemd/system/ic-crypto-csp.socket",
    Label("etc/systemd/system/ic-https-outcalls-adapter.service"): "/etc/systemd/system/ic-https-outcalls-adapter.service",
    Label("etc/systemd/system/ic-https-outcalls-adapter.socket"): "/etc/systemd/system/ic-https-outcalls-adapter.socket",
    Label("etc/systemd/system/ic-replica.service"): "/etc/systemd/system/ic-replica.service",
    Label("etc/systemd/system/ipv4-connectivity-check.service"): "/etc/systemd/system/ipv4-connectivity-check.service",
    Label("etc/systemd/system/ipv4-connectivity-check.timer"): "/etc/systemd/system/ipv4-connectivity-check.timer",
    Label("etc/systemd/system/metrics-proxy.service"): "/etc/systemd/system/metrics-proxy.service",
    Label("etc/systemd/system/monitor-expand-shared-data.service"): "/etc/systemd/system/monitor-expand-shared-data.service",
    Label("etc/systemd/system/node_exporter.service"): "/etc/systemd/system/node_exporter.service",
    Label("etc/systemd/system/relabel-machine-id.service"): "/etc/systemd/system/relabel-machine-id.service",
    Label("etc/systemd/system/reload_nftables.path"): "/etc/systemd/system/reload_nftables.path",
    Label("etc/systemd/system/reload_nftables.service"): "/etc/systemd/system/reload_nftables.service",
    Label("etc/systemd/system/retry-ipv6-config.service"): "/etc/systemd/system/retry-ipv6-config.service",
    Label("etc/systemd/system/save-machine-id.service"): "/etc/systemd/system/save-machine-id.service",
    Label("etc/systemd/system/serial-getty@.service"): "/etc/systemd/system/serial-getty@.service",
    Label("etc/systemd/system/setup-encryption.service"): "/etc/systemd/system/setup-encryption.service",
    Label("etc/systemd/system/setup-fstrim-metrics.service"): "/etc/systemd/system/setup-fstrim-metrics.service",
    Label("etc/systemd/system/setup-hostname.service"): "/etc/systemd/system/setup-hostname.service",
    Label("etc/systemd/system/setup-lvs.service"): "/etc/systemd/system/setup-lvs.service",
    Label("etc/systemd/system/setup-node-gen-status.service"): "/etc/systemd/system/setup-node-gen-status.service",
    Label("etc/systemd/system/setup-node_exporter-keys.service"): "/etc/systemd/system/setup-node_exporter-keys.service",
    Label("etc/systemd/system/setup-permissions.service"): "/etc/systemd/system/setup-permissions.service",
    Label("etc/systemd/system/setup-shared-backup.service"): "/etc/systemd/system/setup-shared-backup.service",
    Label("etc/systemd/system/setup-shared-crypto.service"): "/etc/systemd/system/setup-shared-crypto.service",
    Label("etc/systemd/system/setup-shared-data.service"): "/etc/systemd/system/setup-shared-data.service",
    Label("etc/systemd/system/setup-ssh-account-keys.service"): "/etc/systemd/system/setup-ssh-account-keys.service",
    Label("etc/systemd/system/setup-ssh-keys.service"): "/etc/systemd/system/setup-ssh-keys.service",
    Label("etc/systemd/system/upgrade-shared-data-store.service"): "/etc/systemd/system/upgrade-shared-data-store.service",
    Label("etc/systemd/system/user@.service"): "/etc/systemd/system/user@.service",
    Label("etc/tmpfiles.d/ic-node.conf"): "/etc/tmpfiles.d/ic-node.conf",
    Label("etc/udev/rules.d/10-vhost-vsock.rules"): "/etc/udev/rules.d/10-vhost-vsock.rules",
    Label("opt/ic/bin/bootstrap-ic-node.sh"): "/opt/ic/bin/bootstrap-ic-node.sh",
    Label("opt/ic/bin/generate-btc-adapter-config.sh"): "/opt/ic/bin/generate-btc-adapter-config.sh",
    Label("opt/ic/bin/generate-filebeat-config.sh"): "/opt/ic/bin/generate-filebeat-config.sh",
    Label("opt/ic/bin/generate-https-outcalls-adapter-config.sh"): "/opt/ic/bin/generate-https-outcalls-adapter-config.sh",
    Label("opt/ic/bin/generate-replica-config.sh"): "/opt/ic/bin/generate-replica-config.sh",
    Label("opt/ic/bin/ipv4-connectivity-check.sh"): "/opt/ic/bin/ipv4-connectivity-check.sh",
    Label("opt/ic/bin/manageboot.sh"): "/opt/ic/bin/manageboot.sh",
    Label("opt/ic/bin/metrics.sh"): "/opt/ic/bin/metrics.sh",
    Label("opt/ic/bin/monitor-expand-shared-data.py"): "/opt/ic/bin/monitor-expand-shared-data.py",
    Label("opt/ic/bin/provision-ssh-keys.sh"): "/opt/ic/bin/provision-ssh-keys.sh",
    Label("opt/ic/bin/read-ssh-keys.sh"): "/opt/ic/bin/read-ssh-keys.sh",
    Label("opt/ic/bin/relabel-machine-id.sh"): "/opt/ic/bin/relabel-machine-id.sh",
    Label("opt/ic/bin/retry-ipv6-config.sh"): "/opt/ic/bin/retry-ipv6-config.sh",
    Label("opt/ic/bin/save-machine-id.sh"): "/opt/ic/bin/save-machine-id.sh",
    Label("opt/ic/bin/setup-encryption.sh"): "/opt/ic/bin/setup-encryption.sh",
    Label("opt/ic/bin/setup-filebeat-permissions.sh"): "/opt/ic/bin/setup-filebeat-permissions.sh",
    Label("opt/ic/bin/setup-hostname.sh"): "/opt/ic/bin/setup-hostname.sh",
    Label("opt/ic/bin/setup-lvs.sh"): "/opt/ic/bin/setup-lvs.sh",
    Label("opt/ic/bin/setup-node_exporter-keys.sh"): "/opt/ic/bin/setup-node_exporter-keys.sh",
    Label("opt/ic/bin/setup-permissions.sh"): "/opt/ic/bin/setup-permissions.sh",
    Label("opt/ic/bin/setup-shared-backup.sh"): "/opt/ic/bin/setup-shared-backup.sh",
    Label("opt/ic/bin/setup-shared-crypto.sh"): "/opt/ic/bin/setup-shared-crypto.sh",
    Label("opt/ic/bin/setup-shared-data.sh"): "/opt/ic/bin/setup-shared-data.sh",
    Label("opt/ic/bin/setup-ssh-account-keys.sh"): "/opt/ic/bin/setup-ssh-account-keys.sh",
    Label("opt/ic/bin/setup-ssh-keys.sh"): "/opt/ic/bin/setup-ssh-keys.sh",
    Label("opt/ic/bin/setup-var-encryption.sh"): "/opt/ic/bin/setup-var-encryption.sh",
    Label("opt/ic/bin/upgrade-shared-data-store.sh"): "/opt/ic/bin/upgrade-shared-data-store.sh",
    Label("opt/ic/bin/validate-replica-config.sh"): "/opt/ic/bin/validate-replica-config.sh",
    Label("opt/ic/share/ark.pem"): "/opt/ic/share/ark.pem",
    Label("opt/ic/share/ic.json5.template"): "/opt/ic/share/ic.json5.template",
    Label("prep/filebeat/filebeat.fc"): "/prep/filebeat/filebeat.fc",
    Label("prep/filebeat/filebeat.if"): "/prep/filebeat/filebeat.if",
    Label("prep/filebeat/filebeat.te"): "/prep/filebeat/filebeat.te",
    Label("prep/fscontext-fixes/fscontext-fixes.fc"): "/prep/fscontext-fixes/fscontext-fixes.fc",
    Label("prep/fscontext-fixes/fscontext-fixes.if"): "/prep/fscontext-fixes/fscontext-fixes.if",
    Label("prep/fscontext-fixes/fscontext-fixes.te"): "/prep/fscontext-fixes/fscontext-fixes.te",
    Label("prep/ic-node/ic-node.fc"): "/prep/ic-node/ic-node.fc",
    Label("prep/ic-node/ic-node.if"): "/prep/ic-node/ic-node.if",
    Label("prep/ic-node/ic-node.te"): "/prep/ic-node/ic-node.te",
    Label("prep/infogetty/infogetty.fc"): "/prep/infogetty/infogetty.fc",
    Label("prep/infogetty/infogetty.te"): "/prep/infogetty/infogetty.te",
    Label("prep/manageboot/manageboot.fc"): "/prep/manageboot/manageboot.fc",
    Label("prep/manageboot/manageboot.if"): "/prep/manageboot/manageboot.if",
    Label("prep/manageboot/manageboot.te"): "/prep/manageboot/manageboot.te",
    Label("prep/misc-fixes/misc-fixes.if"): "/prep/misc-fixes/misc-fixes.if",
    Label("prep/misc-fixes/misc-fixes.te"): "/prep/misc-fixes/misc-fixes.te",
    Label("prep/node_exporter/node_exporter.fc"): "/prep/node_exporter/node_exporter.fc",
    Label("prep/node_exporter/node_exporter.if"): "/prep/node_exporter/node_exporter.if",
    Label("prep/node_exporter/node_exporter.te"): "/prep/node_exporter/node_exporter.te",
    Label("prep/prep.sh"): "/prep/prep.sh",
    Label("prep/setup-var/setup-var.if"): "/prep/setup-var/setup-var.if",
    Label("prep/setup-var/setup-var.te"): "/prep/setup-var/setup-var.te",
    Label("prep/systemd-fixes/systemd-fixes.if"): "/prep/systemd-fixes/systemd-fixes.if",
    Label("prep/systemd-fixes/systemd-fixes.te"): "/prep/systemd-fixes/systemd-fixes.te",
}
