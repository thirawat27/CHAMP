/**
 * Audio Manager - Lo-fi Cafe Sound Design
 *
 * Warm, mellow, relaxing sound effects using Web Audio API.
 * Inspired by lo-fi music and cafe ambience - cozy, organic, and calming.
 *
 * Design principles:
 * - All tones stay below 600 Hz (warm register, never shrill)
 * - Triangle + sine blend for organic warmth
 * - Slow attack (≥80 ms) so notes "breathe" in
 * - Long exponential decay for natural resonance tail
 * - Gentle low-pass filter (≤900 Hz cutoff) to remove harshness
 * - Gain kept at 0.18–0.28 — audible but not startling
 */

import { useLanguageStore } from "../stores/languageStore";

// Audio context singleton
let audioContext: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (!audioContext) {
    try {
      audioContext = new (
        window.AudioContext ||
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
      )();
    } catch {
      console.warn("Web Audio API not supported");
      return null;
    }
  }
  if (audioContext.state === "suspended") {
    audioContext.resume().catch(console.error);
  }
  return audioContext;
}

function isSoundEnabled(): boolean {
  return useLanguageStore.getState().soundEnabled;
}

// ---------------------------------------------------------------------------
// Shared helper – plays one warm note (triangle + sine blend through LP filter)
// ---------------------------------------------------------------------------
function playNote(
  ctx: AudioContext,
  freq: number,
  startTime: number,
  attack: number,
  decay: number,
  gain: number
): void {
  // Triangle oscillator – warm body
  const tri = ctx.createOscillator();
  const triGain = ctx.createGain();
  const lp = ctx.createBiquadFilter();

  tri.type = "triangle";
  tri.frequency.setValueAtTime(freq, startTime);

  lp.type = "lowpass";
  lp.frequency.setValueAtTime(700, startTime); // cut everything above 700 Hz
  lp.Q.setValueAtTime(0.4, startTime);

  tri.connect(lp);
  lp.connect(triGain);
  triGain.connect(ctx.destination);

  triGain.gain.setValueAtTime(0, startTime);
  triGain.gain.linearRampToValueAtTime(gain * 0.7, startTime + attack);
  triGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attack + decay);

  tri.start(startTime);
  tri.stop(startTime + attack + decay + 0.05);

  // Sine sub – adds warmth / body without brightness
  const sub = ctx.createOscillator();
  const subGain = ctx.createGain();

  sub.type = "sine";
  sub.frequency.setValueAtTime(freq * 0.5, startTime); // one octave below

  sub.connect(subGain);
  subGain.connect(ctx.destination);

  subGain.gain.setValueAtTime(0, startTime);
  subGain.gain.linearRampToValueAtTime(gain * 0.3, startTime + attack);
  subGain.gain.exponentialRampToValueAtTime(0.0001, startTime + attack + decay * 0.8);

  sub.start(startTime);
  sub.stop(startTime + attack + decay + 0.05);
}

// ---------------------------------------------------------------------------
// Service sounds
// ---------------------------------------------------------------------------

/**
 * START – gentle ascending three-note phrase
 * C3 → E3 → G3  (196 / 247 / 294 Hz)
 * Feels like: "opening up", welcoming
 */
export function playStartSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 196.00, t: 0.00 },  // G3
    { freq: 246.94, t: 0.18 },  // B3
    { freq: 293.66, t: 0.36 },  // D4
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.10, 1.4, 0.24)
  );
}

/**
 * RESTART – two-note "turn" figure
 * D4 → G3  (down a fifth, then back up implied)
 * Feels like: "cycling", refreshing
 */
export function playRestartSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 293.66, t: 0.00 },  // D4
    { freq: 196.00, t: 0.20 },  // G3
    { freq: 293.66, t: 0.40 },  // D4
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.09, 1.1, 0.22)
  );
}

/**
 * STOP – descending three-note phrase, peaceful resolution
 * G3 → E3 → C3  (196 / 165 / 131 Hz)
 * Feels like: "settling down", resting
 */
export function playStopSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 196.00, t: 0.00 },  // G3
    { freq: 164.81, t: 0.20 },  // E3
    { freq: 130.81, t: 0.40 },  // C3
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.12, 1.6, 0.20)
  );
}

/**
 * SUCCESS / COMPLETE – warm four-note ascending arpeggio
 * C3 → E3 → G3 → C4  (131 / 165 / 196 / 262 Hz)
 * Feels like: "done!", satisfying
 */
export function playCompleteSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 130.81, t: 0.00 },  // C3
    { freq: 164.81, t: 0.16 },  // E3
    { freq: 196.00, t: 0.32 },  // G3
    { freq: 261.63, t: 0.48 },  // C4 – final note longer
  ];

  phrase.forEach(({ freq, t }, i) =>
    playNote(ctx, freq, now + t, 0.10, i === 3 ? 2.0 : 1.2, 0.26)
  );
}

/**
 * ERROR – two-note descending minor second, gentle but noticeable
 * E3 → D#3  (165 / 156 Hz) – dissonant but not harsh
 * Feels like: "hmm, something's off"
 */
export function playErrorSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;

  const phrase = [
    { freq: 164.81, t: 0.00 },  // E3
    { freq: 155.56, t: 0.22 },  // Eb3
  ];

  phrase.forEach(({ freq, t }) =>
    playNote(ctx, freq, now + t, 0.10, 1.4, 0.24)
  );
}

// ---------------------------------------------------------------------------
// UI sounds
// ---------------------------------------------------------------------------

/**
 * CLICK – single warm tap, G3
 */
export function playClickSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  playNote(ctx, 196.00, ctx.currentTime, 0.04, 0.22, 0.18);
}

/**
 * TOGGLE – two-note soft click: D4 → A3
 */
export function playToggleSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  const now = ctx.currentTime;
  playNote(ctx, 293.66, now,        0.04, 0.20, 0.16); // D4
  playNote(ctx, 220.00, now + 0.10, 0.04, 0.20, 0.14); // A3
}

/**
 * HOVER – silent (no sound on hover per UX decision)
 */
export function playHoverSound(): void {
  // intentionally silent
}

// ---------------------------------------------------------------------------
// Loading / progress sounds
// ---------------------------------------------------------------------------

/**
 * LOADING PULSE – single soft E3 bell tap
 */
export function playLoadingPulseSound(): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  playNote(ctx, 164.81, ctx.currentTime, 0.06, 0.55, 0.16);
}

/**
 * LOADING STREAM – very subtle progress tone (C3 → G3 range)
 */
export function playLoadingStreamSound(progress: number): void {
  if (!isSoundEnabled()) return;
  const ctx = getAudioContext();
  if (!ctx) return;
  // C3 (131 Hz) rising gently to G3 (196 Hz) as progress goes 0→100
  const freq = 130.81 + (progress / 100) * (196.00 - 130.81);
  playNote(ctx, freq, ctx.currentTime, 0.04, 0.18, 0.08);
}

// ---------------------------------------------------------------------------
// Notification tone – called when a notice banner appears
// ---------------------------------------------------------------------------

/**
 * Play the appropriate sound when a status notification fires.
 *
 * tone    action   sound
 * ------  -------  -----
 * info    start    playStartSound  (pending start)
 * info    restart  playRestartSound
 * info    stop     (silent – don't interrupt)
 * success start    playCompleteSound
 * success restart  playCompleteSound
 * success stop     playStopSound
 * error   *        playErrorSound
 */
export function playNotificationSound(
  tone: "info" | "success" | "error",
  action?: "start" | "restart" | "stop"
): void {
  if (tone === "error") {
    playErrorSound();
    return;
  }
  if (tone === "success") {
    if (action === "stop") {
      playStopSound();
    } else {
      playCompleteSound();
    }
    return;
  }
  // info
  if (action === "start")   { playStartSound();   return; }
  if (action === "restart") { playRestartSound();  return; }
  // info + stop → silent (user already heard the stop click)
}

// ---------------------------------------------------------------------------
// AudioManager object
// ---------------------------------------------------------------------------

export const AudioManager = {
  // Service lifecycle
  playStart:    playStartSound,
  playRestart:  playRestartSound,
  playStop:     playStopSound,
  playComplete: playCompleteSound,

  // Loading / wizard
  playLoadingPulse:  playLoadingPulseSound,
  playLoadingStream: playLoadingStreamSound,

  // UI interactions
  playClick:  playClickSound,
  playToggle: playToggleSound,
  playHover:  playHoverSound,

  // Notification banner
  playNotification: playNotificationSound,

  // Legacy aliases
  playSuccess: playCompleteSound,
  playError:   playErrorSound,

  isSupported: () =>
    typeof window !== "undefined" &&
    !!(window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext),

  initialize: getAudioContext,
};

export default AudioManager;
