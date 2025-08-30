use std::process::{Child, Command};

use color_eyre::eyre::Context;
use color_eyre::Result;

#[derive(Debug)]
pub struct QemuConfig {
    pub memory_mb: u32,
    pub vcpus: u32,
    pub kernel_path: String,
    pub initramfs_path: String,
    pub virtiofs_socket: String,
    pub kernel_cmdline: Vec<String>,
    pub enable_console: bool,
}

pub fn spawn_qemu(config: &QemuConfig) -> Result<Child> {
    let memory_arg = format!("{}M", config.memory_mb);
    let memory_obj_arg = format!(
        "memory-backend-memfd,id=mem,share=on,size={}M",
        config.memory_mb
    );

    let mut cmd = Command::new("qemu-kvm");
    cmd.args([
        "-m",
        &memory_arg,
        "-smp",
        &config.vcpus.to_string(),
        "-enable-kvm",
        "-cpu",
        "host",
        "-kernel",
        &config.kernel_path,
        "-initrd",
        &config.initramfs_path,
        "-chardev",
        &format!("socket,id=char0,path={}", config.virtiofs_socket),
        "-device",
        "vhost-user-fs-pci,queue-size=1024,chardev=char0,tag=rootfs",
        "-object",
        &memory_obj_arg,
        "-numa",
        "node,memdev=mem",
    ]);

    if config.enable_console {
        cmd.args(["-serial", "stdio", "-display", "none"]);
    } else {
        cmd.args(["-display", "none"]);
    }

    let append_str = config.kernel_cmdline.join(" ");
    cmd.args(["-append", &append_str]);

    cmd.spawn().context("Failed to spawn QEMU")
}
