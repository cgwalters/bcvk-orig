use std::io::{Seek as _, Write as _};
use std::os::fd::OwnedFd;

use cap_std_ext::cap_std::io_lifetimes::AsFilelike as _;
use color_eyre::Result;

#[allow(dead_code)]
pub(crate) fn impl_sealed_memfd(description: &str, content: &[u8]) -> Result<OwnedFd> {
    use rustix::fs::{MemfdFlags, SealFlags};
    let mfd =
        rustix::fs::memfd_create(description, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;

    {
        let mfd_file = mfd.as_filelike_view::<std::fs::File>();
        mfd_file.set_len(content.len() as u64)?;
        (&*mfd_file).write_all(content)?;
        (&*mfd_file).seek(std::io::SeekFrom::Start(0))?;
    }

    rustix::fs::fcntl_add_seals(
        &mfd,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
    )?;
    Ok(mfd)
}
