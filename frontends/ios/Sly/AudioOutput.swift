import AVFoundation
import SlyKit

final class AudioOutput {
    private static let int16FullScale = Float(Int16.max) + 1
    private static let startupBufferCount = 3

    private let engine = AVAudioEngine()
    private let playerNode = AVAudioPlayerNode()
    private let format: AVAudioFormat
    private let channelCount: Int
    private let maximumSamplesPerFrame: Int
    private var scheduledBufferCount = 0
    private var hasStartedPlayback = false
    private var isPaused = false

    init?(core: EmulatorCore, framesPerSecond: Int) {
        guard let spec = core.audioSpec,
              let frameIntervalNanoseconds = core.frameIntervalNanoseconds
        else {
            return nil
        }

        channelCount = Int(spec.channels)
        
        let emulatedFrameDuration =
            Double(frameIntervalNanoseconds) / 1_000_000_000
        let playbackSampleRate =
            Double(spec.sampleRate) * emulatedFrameDuration
                * Double(framesPerSecond)
        
        guard spec.sampleRate > 0,
              channelCount > 0,
              framesPerSecond > 0,
              playbackSampleRate.isFinite,
              playbackSampleRate > 0,
              let format = AVAudioFormat(
                  standardFormatWithSampleRate: playbackSampleRate,
                  channels: AVAudioChannelCount(channelCount)
              )
        else {
            return nil
        }

        self.format = format
        let maximumAudioFrames =
            (Int(spec.sampleRate) + framesPerSecond - 1) / framesPerSecond
        maximumSamplesPerFrame = maximumAudioFrames * channelCount

        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.ambient, mode: .default)
            try session.setActive(true)

            engine.attach(playerNode)
            engine.connect(playerNode, to: engine.mainMixerNode, format: format)
            engine.prepare()
            try engine.start()
        } catch {
            engine.stop()
            try? AVAudioSession.sharedInstance().setActive(
                false,
                options: .notifyOthersOnDeactivation
            )
            print("Unable to start audio: \(error)")
            return nil
        }
    }

    func enqueue(from core: EmulatorCore) {
        let samples = core.drainAudio(maxSamples: maximumSamplesPerFrame)
        let frameCount = samples.count / channelCount
        guard frameCount > 0,
              let buffer = AVAudioPCMBuffer(
                  pcmFormat: format,
                  frameCapacity: AVAudioFrameCount(frameCount)
              ),
              let channels = buffer.floatChannelData
        else {
            return
        }

        buffer.frameLength = AVAudioFrameCount(frameCount)
        for frame in 0..<frameCount {
            for channel in 0..<channelCount {
                channels[channel][frame] = Float(
                    samples[frame * channelCount + channel]
                ) / Self.int16FullScale
            }
        }

        playerNode.scheduleBuffer(buffer)
        if !hasStartedPlayback {
            scheduledBufferCount += 1
        }
        startPlaybackIfReady()
    }

    func setPaused(_ isPaused: Bool) {
        self.isPaused = isPaused
        if isPaused {
            playerNode.pause()
        } else {
            startPlaybackIfReady()
        }
    }

    func stop() {
        playerNode.stop()
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(
            false,
            options: .notifyOthersOnDeactivation
        )
    }

    private func startPlaybackIfReady() {
        guard !isPaused,
              !playerNode.isPlaying,
              hasStartedPlayback ||
                  scheduledBufferCount >= Self.startupBufferCount
        else {
            return
        }

        playerNode.play()
        hasStartedPlayback = true
    }
}
