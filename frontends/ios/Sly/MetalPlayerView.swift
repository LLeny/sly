import CoreImage
import MetalKit
import SwiftUI
import SlyKit

struct MetalPlayerView: UIViewRepresentable {
    let player: Player
    let framesPerSecond: Int

    func makeCoordinator() -> Renderer {
        Renderer(player: player)
    }

    func makeUIView(context: Context) -> MTKView {
        let view = MTKView(frame: .zero, device: context.coordinator.device)
        view.delegate = context.coordinator
        view.colorPixelFormat = .bgra8Unorm
        view.framebufferOnly = false
        view.preferredFramesPerSecond = framesPerSecond
        view.enableSetNeedsDisplay = false
        view.isPaused = false
        return view
    }

    func updateUIView(_ view: MTKView, context: Context) {
        context.coordinator.player = player
        view.preferredFramesPerSecond = framesPerSecond
    }

    static func dismantleUIView(_ view: MTKView, coordinator: Renderer) {
        view.isPaused = true
        view.delegate = nil
    }

    final class Renderer: NSObject, MTKViewDelegate {
        var player: Player

        let device: MTLDevice

        private let commandQueue: MTLCommandQueue
        private let context: CIContext
        private let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!
        private var frameTexture: MTLTexture?

        init(player: Player) {
            guard let device = MTLCreateSystemDefaultDevice(),
                  let commandQueue = device.makeCommandQueue()
            else {
                fatalError("Metal is unavailable")
            }

            self.player = player
            self.device = device
            self.commandQueue = commandQueue
            context = CIContext(
                mtlDevice: device,
                options: [.cacheIntermediates: false]
            )
        }

        func draw(in view: MTKView) {
            guard let frame = player.runFrame(),
                  upload(frame),
                  let frameTexture,
                  let textureImage = CIImage(
                      mtlTexture: frameTexture,
                      options: [.colorSpace: colorSpace]
                  ),
                  let drawable = view.currentDrawable,
                  let commandBuffer = commandQueue.makeCommandBuffer()
            else {
                return
            }

            let image = textureImage.transformed(
                by: CGAffineTransform(
                    a: 1,
                    b: 0,
                    c: 0,
                    d: -1,
                    tx: 0,
                    ty: textureImage.extent.height
                )
            )

            let bounds = CGRect(
                origin: .zero,
                size: CGSize(
                    width: view.drawableSize.width,
                    height: view.drawableSize.height
                )
            )
            guard !bounds.isEmpty else { return }

            let scale = min(
                bounds.width / image.extent.width,
                bounds.height / image.extent.height
            )
            let transform = CGAffineTransform(
                a: scale,
                b: 0,
                c: 0,
                d: scale,
                tx: (bounds.width - image.extent.width * scale) / 2,
                ty: (bounds.height - image.extent.height * scale) / 2
            )
            let background = CIImage(color: .black).cropped(to: bounds)
            let output = image
                .transformed(by: transform)
                .composited(over: background)

            context.render(
                output,
                to: drawable.texture,
                commandBuffer: commandBuffer,
                bounds: bounds,
                colorSpace: colorSpace
            )
            commandBuffer.present(drawable)
            commandBuffer.commit()
        }

        func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

        private func upload(_ frame: Frame) -> Bool {
            guard frame.width > 0,
                  frame.height > 0,
                  frame.rgba.count == frame.width * frame.height * 4
            else {
                return false
            }

            if frameTexture?.width != frame.width ||
                frameTexture?.height != frame.height {
                let descriptor = MTLTextureDescriptor.texture2DDescriptor(
                    pixelFormat: .rgba8Unorm,
                    width: frame.width,
                    height: frame.height,
                    mipmapped: false
                )
                descriptor.storageMode = .shared
                descriptor.usage = .shaderRead
                frameTexture = device.makeTexture(descriptor: descriptor)
            }

            guard let frameTexture else { return false }

            frame.rgba.withUnsafeBytes { bytes in
                guard let address = bytes.baseAddress else { return }
                frameTexture.replace(
                    region: MTLRegionMake2D(0, 0, frame.width, frame.height),
                    mipmapLevel: 0,
                    withBytes: address,
                    bytesPerRow: frame.width * 4
                )
            }
            return true
        }
    }
}
