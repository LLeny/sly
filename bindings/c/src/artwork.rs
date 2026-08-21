use core_model::{Region, System};
use snes::Cartridge;

use crate::capi::{
    SlyBuffer, SlyStatus, SlySystem, buffer_from_vec, set_last_error, slice_from_raw,
};
use crate::runtime::system_from_index;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlyRegion {
    Japan = 0,
    NorthAmerica,
    Europe,
    Scandinavia,
    Finland,
    Denmark,
    France,
    Netherlands,
    Spain,
    Germany,
    Italy,
    China,
    Indonesia,
    SouthKorea,
    Common,
    Canada,
    Brazil,
    Australia,
    Other,
}

/// Rust-allocated fields. Release them exactly once with `sly_artwork_query_free`.
#[repr(C)]
#[derive(Debug)]
pub struct SlyArtworkQuery {
    pub sha1: SlyBuffer,
    pub metadata_url: SlyBuffer,
    pub authorization: SlyBuffer,
    pub region: SlyRegion,
}

/// Rust-allocated UTF-8 fields. Empty artwork URL fields mean unavailable.
/// Release them exactly once with `sly_artwork_manifest_free`.
#[repr(C)]
#[derive(Debug)]
pub struct SlyArtworkManifest {
    pub title: SlyBuffer,
    pub front_url: SlyBuffer,
    pub back_url: SlyBuffer,
    pub spine_url: SlyBuffer,
    pub label_url: SlyBuffer,
}

/// Inspect a ROM without constructing an emulator core and prepare a art.layle.dev artwork query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_artwork_query(
    system: SlySystem,
    rom: *const u8,
    rom_len: usize,
    out_query: *mut SlyArtworkQuery,
) -> SlyStatus {
    if out_query.is_null() || (rom.is_null() && rom_len > 0) {
        set_last_error("NULL argument to sly_artwork_query");
        return SlyStatus::InvalidArg;
    }

    let Some(system) = system_from_index(system as usize) else {
        set_last_error("unknown artwork system");
        return SlyStatus::InvalidArg;
    };
    let bytes = unsafe { slice_from_raw(rom, rom_len) };

    let region = match inspect_region(system, bytes) {
        Ok(region) => region,
        Err(err) => {
            set_last_error(&err);
            return SlyStatus::BadRom;
        }
    };
    let sha1 = ::artwork::rom_sha1(system, bytes);
    let query = SlyArtworkQuery {
        metadata_url: buffer_from_string(::artwork::layle::metadata_url(&sha1)),
        authorization: buffer_from_string(::artwork::layle::authorization()),
        sha1: buffer_from_string(sha1),
        region: region.into(),
    };
    unsafe { out_query.write(query) };

    SlyStatus::Ok
}

/// Resolve art.layle.dev metadata into region-preferred artwork URLs without performing network I/O.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_artwork_resolve(
    sha1: *const u8,
    sha1_len: usize,
    preferred_region: SlyRegion,
    rom_region: SlyRegion,
    metadata: *const u8,
    metadata_len: usize,
    out_manifest: *mut SlyArtworkManifest,
) -> SlyStatus {
    if out_manifest.is_null()
        || (sha1.is_null() && sha1_len > 0)
        || (metadata.is_null() && metadata_len > 0)
    {
        set_last_error("NULL argument to sly_artwork_resolve");
        return SlyStatus::InvalidArg;
    }

    let sha1_bytes = unsafe { slice_from_raw(sha1, sha1_len) };
    let Ok(sha1) = std::str::from_utf8(sha1_bytes) else {
        set_last_error("artwork SHA-1 is not UTF-8");
        return SlyStatus::InvalidArg;
    };
    if sha1.len() != 40 || !sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        set_last_error("artwork SHA-1 must contain 40 hexadecimal characters");
        return SlyStatus::InvalidArg;
    }
    let metadata = unsafe { slice_from_raw(metadata, metadata_len) };

    match ::artwork::layle::resolve_metadata(
        sha1,
        &[preferred_region.into(), rom_region.into()],
        metadata,
    ) {
        Ok(manifest) => {
            unsafe {
                out_manifest.write(SlyArtworkManifest {
                    title: buffer_from_string(manifest.title),
                    front_url: buffer_from_option(manifest.front),
                    back_url: buffer_from_option(manifest.back),
                    spine_url: buffer_from_option(manifest.spine),
                    label_url: buffer_from_option(manifest.label),
                })
            };
            SlyStatus::Ok
        }
        Err(err) => {
            set_last_error(&format!("invalid artwork metadata: {err}"));
            SlyStatus::ArtworkMetadata
        }
    }
}

/// NULL is a no-op. Invalidates every field in the query.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_artwork_query_free(query: *mut SlyArtworkQuery) {
    let Some(query) = (unsafe { query.as_mut() }) else {
        return;
    };
    unsafe {
        crate::capi::sly_buffer_free(&mut query.sha1);
        crate::capi::sly_buffer_free(&mut query.metadata_url);
        crate::capi::sly_buffer_free(&mut query.authorization);
    }
}

/// NULL is a no-op. Invalidates every field in the manifest.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sly_artwork_manifest_free(manifest: *mut SlyArtworkManifest) {
    let Some(manifest) = (unsafe { manifest.as_mut() }) else {
        return;
    };
    unsafe {
        crate::capi::sly_buffer_free(&mut manifest.title);
        crate::capi::sly_buffer_free(&mut manifest.front_url);
        crate::capi::sly_buffer_free(&mut manifest.back_url);
        crate::capi::sly_buffer_free(&mut manifest.spine_url);
        crate::capi::sly_buffer_free(&mut manifest.label_url);
    }
}

fn inspect_region(system: System, rom: &[u8]) -> Result<Region, String> {
    match system {
        System::Snes => Cartridge::region_of(rom).map_err(|err| err.to_string()),
    }
}

fn buffer_from_string(value: String) -> SlyBuffer {
    buffer_from_vec(value.into_bytes())
}

fn buffer_from_option(value: Option<String>) -> SlyBuffer {
    buffer_from_string(value.unwrap_or_default())
}

impl From<Region> for SlyRegion {
    fn from(region: Region) -> Self {
        match region {
            Region::Japan => Self::Japan,
            Region::NorthAmerica => Self::NorthAmerica,
            Region::Europe => Self::Europe,
            Region::Scandinavia => Self::Scandinavia,
            Region::Finland => Self::Finland,
            Region::Denmark => Self::Denmark,
            Region::France => Self::France,
            Region::Netherlands => Self::Netherlands,
            Region::Spain => Self::Spain,
            Region::Germany => Self::Germany,
            Region::Italy => Self::Italy,
            Region::China => Self::China,
            Region::Indonesia => Self::Indonesia,
            Region::SouthKorea => Self::SouthKorea,
            Region::Common => Self::Common,
            Region::Canada => Self::Canada,
            Region::Brazil => Self::Brazil,
            Region::Australia => Self::Australia,
            Region::Other(_) => Self::Other,
        }
    }
}

impl From<SlyRegion> for Region {
    fn from(region: SlyRegion) -> Self {
        match region {
            SlyRegion::Japan => Self::Japan,
            SlyRegion::NorthAmerica => Self::NorthAmerica,
            SlyRegion::Europe => Self::Europe,
            SlyRegion::Scandinavia => Self::Scandinavia,
            SlyRegion::Finland => Self::Finland,
            SlyRegion::Denmark => Self::Denmark,
            SlyRegion::France => Self::France,
            SlyRegion::Netherlands => Self::Netherlands,
            SlyRegion::Spain => Self::Spain,
            SlyRegion::Germany => Self::Germany,
            SlyRegion::Italy => Self::Italy,
            SlyRegion::China => Self::China,
            SlyRegion::Indonesia => Self::Indonesia,
            SlyRegion::SouthKorea => Self::SouthKorea,
            SlyRegion::Common => Self::Common,
            SlyRegion::Canada => Self::Canada,
            SlyRegion::Brazil => Self::Brazil,
            SlyRegion::Australia => Self::Australia,
            SlyRegion::Other => Self::Other(0xff),
        }
    }
}
