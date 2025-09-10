//! Integration tests for libvirt-upload-disk command
//!
//! These tests verify the libvirt disk upload functionality, including:
//! - Disk image creation via run-install
//! - Upload to libvirt storage pools
//! - Container image metadata annotation
//! - Error handling and validation

use std::process::Command;

use crate::get_bck_command;
