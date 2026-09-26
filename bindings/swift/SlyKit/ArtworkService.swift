import Foundation

@_implementationOnly import SlyFFI

public struct ArtworkCover: Sendable {
    public let title: String
    public let data: Data
}

public enum ArtworkError: Error, Equatable {
    case invalidURL(String)
    case invalidResponse
    case httpStatus(Int)
}

public enum ArtworkRegion: String, CaseIterable, Identifiable, Sendable {
    case automatic
    case northAmerica
    case europe
    case japan

    public var id: Self { self }

    public var displayName: String {
        switch self {
        case .automatic: "Automatic"
        case .northAmerica: "North America"
        case .europe: "Europe"
        case .japan: "Japan"
        }
    }

    fileprivate func native(romRegion: SlyRegion) -> SlyRegion {
        switch self {
        case .automatic: romRegion
        case .northAmerica: SLY_REGION_NORTH_AMERICA
        case .europe: SLY_REGION_EUROPE
        case .japan: SLY_REGION_JAPAN
        }
    }
}

public actor ArtworkService {
    public static let shared = ArtworkService()

    private let session: URLSession
    private let covers = NSCache<NSString, CachedArtworkCover>()
    private let diskCache = ArtworkDiskCache()
    private var unavailableCovers: Set<String> = []
    private var queries: [String: NativeArtworkQuery] = [:]

    public init(session: URLSession = .shared) {
        self.session = session
        covers.countLimit = 64
        covers.totalCostLimit = 64 * 1_024 * 1_024
    }

    public func cover(
        system: System,
        romURL: URL,
        preferredRegion: ArtworkRegion = .automatic
    ) async throws -> ArtworkCover? {
        try await cover(
            query: try query(system: system, romURL: romURL),
            preferredRegion: preferredRegion
        )
    }

    public func cover(
        system: System,
        rom: Data,
        preferredRegion: ArtworkRegion = .automatic
    ) async throws -> ArtworkCover? {
        try await cover(
            query: try NativeArtwork.query(system: system, rom: rom),
            preferredRegion: preferredRegion
        )
    }

    private func query(system: System, romURL: URL) throws -> NativeArtworkQuery {
        let stamp = try? romURL.resourceValues(
            forKeys: [.fileSizeKey, .contentModificationDateKey]
        )
        let key = [
            system.slug,
            romURL.path,
            String(stamp?.fileSize ?? -1),
            String(stamp?.contentModificationDate?.timeIntervalSince1970 ?? -1),
        ].joined(separator: "|")
        if let cached = queries[key] {
            return cached
        }

        let rom = try Data(contentsOf: romURL, options: .mappedIfSafe)
        let query = try NativeArtwork.query(system: system, rom: rom)
        queries[key] = query
        return query
    }

    private func cover(
        query: NativeArtworkQuery,
        preferredRegion: ArtworkRegion
    ) async throws -> ArtworkCover? {
        let region = preferredRegion.native(romRegion: query.region)
        let cacheKey = "\(query.system.slug)-\(query.sha1)-\(preferredRegion.rawValue)"
        if let cached = covers.object(forKey: cacheKey as NSString) {
            return cached.cover
        }
        if let cached = diskCache.cover(forKey: cacheKey) {
            covers.setObject(
                CachedArtworkCover(cached),
                forKey: cacheKey as NSString,
                cost: cached.data.count
            )
            return cached
        }
        if unavailableCovers.contains(cacheKey) {
            return nil
        }

        let metadata: Data
        do {
            metadata = try await data(
                from: query.metadataURL,
                authorization: query.authorization
            )
        } catch ArtworkError.httpStatus(404) {
            unavailableCovers.insert(cacheKey)
            return nil
        }
        let manifest = try NativeArtwork.resolve(
            query: query,
            region: region,
            metadata: metadata
        )
        guard let frontURL = manifest.frontURL else {
            unavailableCovers.insert(cacheKey)
            return nil
        }

        let image = try await data(
            from: frontURL,
            authorization: query.authorization
        )
        let cover = ArtworkCover(title: manifest.title, data: image)
        diskCache.store(cover, forKey: cacheKey)
        covers.setObject(
            CachedArtworkCover(cover),
            forKey: cacheKey as NSString,
            cost: image.count
        )
        return cover
    }

    private func data(from url: URL, authorization: String) async throws -> Data {
        var request = URLRequest(
            url: url,
            cachePolicy: .returnCacheDataElseLoad,
            timeoutInterval: 15
        )
        request.setValue(authorization, forHTTPHeaderField: "Authorization")

        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse else {
            throw ArtworkError.invalidResponse
        }
        guard (200..<300).contains(response.statusCode) else {
            throw ArtworkError.httpStatus(response.statusCode)
        }
        return data
    }
}

private struct ArtworkDiskCache {
    private let directory: URL

    init(fileManager: FileManager = .default) {
        let baseDirectory = fileManager.urls(
            for: .cachesDirectory,
            in: .userDomainMask
        ).first ?? fileManager.temporaryDirectory
        directory = baseDirectory
            .appendingPathComponent("Sly", isDirectory: true)
            .appendingPathComponent("Artwork", isDirectory: true)
    }

    func cover(forKey key: String) -> ArtworkCover? {
        let url = fileURL(forKey: key)
        do {
            let data = try Data(contentsOf: url)
            return try PropertyListDecoder()
                .decode(StoredArtworkCover.self, from: data)
                .cover
        } catch CocoaError.fileReadNoSuchFile {
            return nil
        } catch {
            try? FileManager.default.removeItem(at: url)
            return nil
        }
    }

    func store(_ cover: ArtworkCover, forKey key: String) {
        do {
            try FileManager.default.createDirectory(
                at: directory,
                withIntermediateDirectories: true
            )
            let encoder = PropertyListEncoder()
            encoder.outputFormat = .binary
            let data = try encoder.encode(StoredArtworkCover(cover))
            try data.write(to: fileURL(forKey: key), options: .atomic)
        } catch {
            // Artwork remains usable from memory if the system cache is unavailable!
        }
    }

    private func fileURL(forKey key: String) -> URL {
        directory.appendingPathComponent("v1-\(key).plist")
    }
}

private struct StoredArtworkCover: Codable {
    let title: String
    let data: Data

    init(_ cover: ArtworkCover) {
        title = cover.title
        data = cover.data
    }

    var cover: ArtworkCover {
        ArtworkCover(title: title, data: data)
    }
}

private struct NativeArtworkQuery {
    let system: System
    let sha1: String
    let metadataURL: URL
    let authorization: String
    let region: SlyRegion
}

private struct NativeArtworkManifest {
    let title: String
    let frontURL: URL?
}

private final class CachedArtworkCover: NSObject {
    let cover: ArtworkCover

    init(_ cover: ArtworkCover) {
        self.cover = cover
    }
}

private enum NativeArtwork {
    static func query(system: System, rom: Data) throws -> NativeArtworkQuery {
        var native = SlyArtworkQuery(
            sha1: emptyBuffer,
            metadata_url: emptyBuffer,
            authorization: emptyBuffer,
            region: SLY_REGION_OTHER
        )
        let status = rom.withUnsafeBytes { bytes in
            sly_artwork_query(
                system.native,
                bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                bytes.count,
                &native
            )
        }
        try checkNativeStatus(status)
        defer { sly_artwork_query_free(&native) }

        let metadataURLString = string(native.metadata_url)
        guard let metadataURL = URL(string: metadataURLString) else {
            throw ArtworkError.invalidURL(metadataURLString)
        }
        return NativeArtworkQuery(
            system: system,
            sha1: string(native.sha1),
            metadataURL: metadataURL,
            authorization: string(native.authorization),
            region: native.region
        )
    }

    static func resolve(
        query: NativeArtworkQuery,
        region: SlyRegion,
        metadata: Data
    ) throws -> NativeArtworkManifest {
        var native = SlyArtworkManifest(
            title: emptyBuffer,
            front_url: emptyBuffer,
            back_url: emptyBuffer,
            spine_url: emptyBuffer,
            label_url: emptyBuffer
        )
        let sha1 = Data(query.sha1.utf8)
        let status = sha1.withUnsafeBytes { sha1Bytes in
            metadata.withUnsafeBytes { metadataBytes in
                sly_artwork_resolve(
                    query.system.native,
                    sha1Bytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    sha1Bytes.count,
                    region,
                    query.region,
                    metadataBytes.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    metadataBytes.count,
                    &native
                )
            }
        }
        try checkNativeStatus(status)
        defer { sly_artwork_manifest_free(&native) }

        return NativeArtworkManifest(
            title: string(native.title),
            frontURL: url(native.front_url)
        )
    }

    private static var emptyBuffer: SlyBuffer {
        SlyBuffer(data: nil, len: 0)
    }

    private static func string(_ buffer: SlyBuffer) -> String {
        guard buffer.len > 0, let bytes = buffer.data else { return "" }
        return String(
            decoding: UnsafeBufferPointer(start: bytes, count: buffer.len),
            as: UTF8.self
        )
    }

    private static func url(_ buffer: SlyBuffer) -> URL? {
        let value = string(buffer)
        return value.isEmpty ? nil : URL(string: value)
    }
}
