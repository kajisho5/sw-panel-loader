//! 512バイト単位でしか読み書きできないもの（Windowsの生ディスク）を、
//! 好きな位置で読み書きできるように包む。FAT32を作る処理から使う。

use std::io::{self, Read, Seek, SeekFrom, Write};

pub const SECTOR: u64 = 512;

pub struct SectorIo<T: Read + Write + Seek> {
    inner: T,
    pos: u64,
    len: u64,
}

impl<T: Read + Write + Seek> SectorIo<T> {
    pub fn new(inner: T, len: u64) -> Self {
        SectorIo { inner, pos: 0, len }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    fn read_sectors(&mut self, first: u64, count: u64) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; (count * SECTOR) as usize];
        self.inner.seek(SeekFrom::Start(first * SECTOR))?;
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn write_sectors(&mut self, first: u64, data: &[u8]) -> io::Result<()> {
        debug_assert!(data.len() as u64 % SECTOR == 0);
        self.inner.seek(SeekFrom::Start(first * SECTOR))?;
        self.inner.write_all(data)
    }
}

impl<T: Read + Write + Seek> Read for SectorIo<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || buf.is_empty() {
            return Ok(0);
        }
        let n = (buf.len() as u64).min(self.len - self.pos);
        let first = self.pos / SECTOR;
        let last = (self.pos + n - 1) / SECTOR;
        let data = self.read_sectors(first, last - first + 1)?;
        let off = (self.pos - first * SECTOR) as usize;
        buf[..n as usize].copy_from_slice(&data[off..off + n as usize]);
        self.pos += n;
        Ok(n as usize)
    }
}

impl<T: Read + Write + Seek> Write for SectorIo<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.pos >= self.len {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "カードの終わりを超えています"));
        }
        let n = (buf.len() as u64).min(self.len - self.pos);
        let first = self.pos / SECTOR;
        let last = (self.pos + n - 1) / SECTOR;
        let count = last - first + 1;
        let off = (self.pos - first * SECTOR) as usize;
        let aligned = off == 0 && n % SECTOR == 0;
        let mut data = if aligned {
            vec![0u8; (count * SECTOR) as usize]
        } else {
            self.read_sectors(first, count)?
        };
        data[off..off + n as usize].copy_from_slice(&buf[..n as usize]);
        self.write_sectors(first, &data)?;
        self.pos += n;
        Ok(n as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<T: Read + Write + Seek> Seek for SectorIo<T> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let p = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if p < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "負の位置"));
        }
        self.pos = p as u64;
        Ok(self.pos)
    }
}

/// FAT32で作り直す（storage はパーティションの先頭から始まる生の領域）
pub fn format_fat32<T: Read + Write + Seek>(storage: T, len: u64, label: &str) -> io::Result<()> {
    let mut io = SectorIo::new(storage, len);
    let mut lbl = [b' '; 11];
    for (i, b) in label.bytes().filter(|b| b.is_ascii_graphic()).take(11).enumerate() {
        lbl[i] = b.to_ascii_uppercase();
    }
    let opts = fatfs::FormatVolumeOptions::new()
        .fat_type(fatfs::FatType::Fat32)
        .bytes_per_sector(SECTOR as u16)
        .total_sectors((len / SECTOR) as u32)
        .bytes_per_cluster(cluster_size(len))
        .volume_label(lbl);
    fatfs::format_volume(&mut io, opts)?;
    io.flush()
}

/// 容量に合わせたクラスタの大きさ（Windowsの標準に近い値）
fn cluster_size(len: u64) -> u32 {
    const GB: u64 = 1024 * 1024 * 1024;
    match len {
        l if l <= 8 * GB => 4096,
        l if l <= 16 * GB => 8192,
        l if l <= 32 * GB => 16384,
        _ => 32768,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// 512単位でないと失敗する偽ディスク
    struct StrictDisk(Cursor<Vec<u8>>);
    impl Read for StrictDisk {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            assert!(b.len() % 512 == 0 && self.0.position() % 512 == 0, "位置か長さがずれています");
            self.0.read(b)
        }
    }
    impl Write for StrictDisk {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            assert!(b.len() % 512 == 0 && self.0.position() % 512 == 0, "位置か長さがずれています");
            self.0.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Seek for StrictDisk {
        fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
            self.0.seek(p)
        }
    }

    #[test]
    fn unaligned_roundtrip() {
        let disk = StrictDisk(Cursor::new(vec![0u8; 4096]));
        let mut io = SectorIo::new(disk, 4096);
        io.seek(SeekFrom::Start(510)).unwrap();
        io.write_all(&[1, 2, 3, 4, 5]).unwrap();
        io.seek(SeekFrom::Start(509)).unwrap();
        let mut b = [9u8; 7];
        io.read_exact(&mut b).unwrap();
        assert_eq!(b, [0, 1, 2, 3, 4, 5, 0]);
    }

    #[test]
    fn makes_valid_fat32() {
        let len: u64 = 300 * 1024 * 1024; // FAT32の最小に近い大きさ
        let disk = StrictDisk(Cursor::new(vec![0u8; len as usize]));
        let mut io = SectorIo::new(disk, len);
        {
            let mut lbl = [b' '; 11];
            lbl[..5].copy_from_slice(b"TURZX");
            let opts = fatfs::FormatVolumeOptions::new()
                .fat_type(fatfs::FatType::Fat32)
                .total_sectors((len / 512) as u32)
                .bytes_per_cluster(4096)
                .volume_label(lbl);
            fatfs::format_volume(&mut io, opts).unwrap();
        }
        io.seek(SeekFrom::Start(0)).unwrap();
        let fs = fatfs::FileSystem::new(&mut io, fatfs::FsOptions::new()).unwrap();
        assert_eq!(fs.fat_type(), fatfs::FatType::Fat32);
        fs.root_dir().create_dir("img").unwrap();
    }
}
