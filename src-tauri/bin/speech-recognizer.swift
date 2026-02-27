import Foundation
import AVFoundation
import Speech

// MARK: - Helper Types

struct AudioLevelEvent: Codable {
    let type: String
    let level: Float
    
    init(level: Float) {
        self.type = "audio-level"
        self.level = level
    }
}

struct TranscriptionEvent: Codable {
    let type: String
    let text: String
    let isFinal: Bool
    
    init(text: String, isFinal: Bool) {
        self.type = "transcription"
        self.text = text
        self.isFinal = isFinal
    }
}

struct ErrorEvent: Codable {
    let type: String
    let message: String
    
    init(message: String) {
        self.type = "error"
        self.message = message
    }
}

// MARK: - Main Logic

class SpeechRecognizer: NSObject {
    private let enableSpeech: Bool
    private let permissionsOnly: Bool
    private let speechRecognizer: SFSpeechRecognizer?
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private let audioEngine = AVAudioEngine()
    private var audioFile: AVAudioFile?
    private let outputFilePath: String
    private var hasStartedRecording = false
    private var shouldStop = false

    init(outputFilePath: String, locale: String, enableSpeech: Bool, permissionsOnly: Bool) {
        self.outputFilePath = outputFilePath
        self.enableSpeech = enableSpeech
        self.permissionsOnly = permissionsOnly
        if enableSpeech {
            self.speechRecognizer = SFSpeechRecognizer(locale: Locale(identifier: locale))
        } else {
            self.speechRecognizer = nil
        }
        super.init()
        debugLog("Initialized with output: \(outputFilePath), locale: \(locale)")
        debugLog("Speech recognition enabled: \(enableSpeech)")
        debugLog("Permissions-only mode: \(permissionsOnly)")
    }

    func start() {
        setbuf(stdout, nil) // Disable stdout buffering

        let bundle = Bundle.main
        let bundlePath = bundle.bundlePath
        let bundleId = bundle.bundleIdentifier ?? "(nil)"
        let speechUsage = bundle.object(forInfoDictionaryKey: "NSSpeechRecognitionUsageDescription") as? String
        let micUsage = bundle.object(forInfoDictionaryKey: "NSMicrophoneUsageDescription") as? String
        debugLog("Bundle path: \(bundlePath)")
        debugLog("Bundle id: \(bundleId)")
        debugLog("Has NSSpeechRecognitionUsageDescription: \(speechUsage != nil)")
        debugLog("Has NSMicrophoneUsageDescription: \(micUsage != nil)")
        
        if enableSpeech {
            guard speechRecognizer != nil else {
                printError("Speech recognizer is not available for the specified locale.")
                exit(1)
            }
            if let recognizer = speechRecognizer, !recognizer.isAvailable {
                debugLog("Speech recognizer is currently unavailable, but continuing to request permission.")
            }

            debugLog("Requesting speech recognition authorization...")
            SFSpeechRecognizer.requestAuthorization { authStatus in
                switch authStatus {
                case .authorized:
                    self.debugLog("Speech authorization granted.")
                    self.requestMicrophoneAuthorizationAndStart()
                case .denied:
                    self.printError("User denied access to speech recognition")
                    exit(1)
                case .restricted:
                    self.printError("Speech recognition restricted on this device")
                    exit(1)
                case .notDetermined:
                    self.printError("Speech recognition not determined")
                    exit(1)
                @unknown default:
                    self.printError("Unknown authorization status")
                    exit(1)
                }
            }
        } else {
            self.requestMicrophoneAuthorizationAndStart()
        }
    }

    private func requestMicrophoneAuthorizationAndStart() {
        debugLog("Requesting microphone authorization...")
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            if granted {
                self.debugLog("Microphone authorization granted.")
                if self.permissionsOnly {
                    self.debugLog("Permissions-only check completed successfully.")
                    exit(0)
                }
                DispatchQueue.main.async {
                    if self.shouldStop {
                        self.debugLog("Stop requested before start; exiting.")
                        exit(0)
                    }
                    do {
                        try self.startRecording()
                    } catch {
                        self.printError("Failed to start recording: \(error)")
                        exit(1)
                    }
                }
            } else {
                self.printError("User denied access to microphone")
                exit(1)
            }
        }
    }

    private func startRecording() throws {
        recognitionTask?.cancel()
        self.recognitionTask = nil

        // On macOS, AVAudioSession is not needed. AVAudioEngine works directly.

        if enableSpeech {
            recognitionRequest = SFSpeechAudioBufferRecognitionRequest()
            guard let recognitionRequest = recognitionRequest else {
                throw NSError(domain: "SpeechRecognizer", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unable to create recognition request"])
            }
            recognitionRequest.shouldReportPartialResults = true
        } else {
            recognitionRequest = nil
        }

        let inputNode = audioEngine.inputNode
        let recordingFormat = inputNode.outputFormat(forBus: 0)
        
        // Write using the device's native format to avoid converter crashes.
        let url = URL(fileURLWithPath: outputFilePath)
        audioFile = try AVAudioFile(
            forWriting: url,
            settings: recordingFormat.settings,
            commonFormat: recordingFormat.commonFormat,
            interleaved: recordingFormat.isInterleaved
        )

        // Tap the input node
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: recordingFormat) { (buffer, when) in
            // 1. Append to recognition request (if enabled)
            if self.enableSpeech {
                self.recognitionRequest?.append(buffer)
            }
            
            // 2. Write to file in native format
            try? self.audioFile?.write(from: buffer)
            
            // 3. Audio Level
            self.calculateAndPrintAudioLevel(buffer: buffer)
        }

        debugLog("Starting audio engine...")
        audioEngine.prepare()
        try audioEngine.start()
        hasStartedRecording = true
        debugLog("Audio engine started.")

        if enableSpeech, let recognitionRequest = recognitionRequest {
            recognitionTask = speechRecognizer?.recognitionTask(with: recognitionRequest) { result, error in
                var isFinal = false
                
                if let result = result {
                    self.printTranscription(result.bestTranscription.formattedString, isFinal: result.isFinal)
                    isFinal = result.isFinal
                }
                
                if error != nil || isFinal {
                    // Keep recording running until stopped by user via stdin.
                    if error != nil {
                        // Ignore recognition errors for now.
                    }
                }
            }
        }
    }
    
    private func stopRecording() {
        if !hasStartedRecording {
            shouldStop = true
            debugLog("Stop requested before recording started; exiting.")
            exit(0)
        }

        debugLog("Stopping recording...")
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionRequest?.endAudio()
        recognitionTask?.cancel()
        audioFile = nil 
        
        debugLog("Recording stopped gracefully.")
        exit(0)
    }
    
    // MARK: - Utils

    private func calculateAndPrintAudioLevel(buffer: AVAudioPCMBuffer) {
        guard let channelData = buffer.floatChannelData else { return }
        let channelDataValue = channelData.pointee
        let channelDataValueArray = stride(from: 0, to: Int(buffer.frameLength), by: buffer.stride).map{ channelDataValue[$0] }
        
        let rms = sqrt(channelDataValueArray.map{ $0 * $0 }.reduce(0, +) / Float(buffer.frameLength))
        let avgPower = 20 * log10(rms)
        
        let minDb: Float = -60.0
        let maxDb: Float = -10.0
        
        var level = (avgPower - minDb) / (maxDb - minDb)
        level = max(0.0, min(1.0, level))
        
        printJSON(AudioLevelEvent(level: level))
    }

    private func printTranscription(_ text: String, isFinal: Bool) {
        printJSON(TranscriptionEvent(text: text, isFinal: isFinal))
    }

    private func debugLog(_ message: String) {
        printJSON(["type": "debug", "message": message])
    }

    private func printError(_ message: String) {
        printJSON(ErrorEvent(message: message))
        fputs("Error: \(message)\n", stderr)
    }
    
    private func printJSON<T: Encodable>(_ data: T) {
        let encoder = JSONEncoder()
        if let jsonData = try? encoder.encode(data), let jsonString = String(data: jsonData, encoding: .utf8) {
            print(jsonString)
            fflush(stdout)
        }
    }
    
    func startMonitoringStdin() {
        DispatchQueue.global().async {
            while let _ = readLine() {
                DispatchQueue.main.async {
                    self.stopRecording()
                }
                break
            }
        }
    }
}

// MARK: - Entry Point

guard CommandLine.arguments.count > 1 else {
    print("Usage: speech-recognizer <output-wav-path> [locale] [--speech] [--permissions-only]")
    exit(1)
}

let outputPath = CommandLine.arguments[1]
let locale = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "ja-JP"
let enableSpeech = CommandLine.arguments.contains("--speech")
let permissionsOnly = CommandLine.arguments.contains("--permissions-only")

if #available(macOS 10.15, *) {
    let recognizer = SpeechRecognizer(
        outputFilePath: outputPath,
        locale: locale,
        enableSpeech: enableSpeech,
        permissionsOnly: permissionsOnly
    )
    recognizer.start()
    if !permissionsOnly {
        recognizer.startMonitoringStdin()
    }
    
    RunLoop.main.run()
} else {
    print("Requires macOS 10.15 or later")
    exit(1)
}
