// Короткие синтезированные звуки интерфейса (Web Audio API).
// Без внешних аудиофайлов — генерируются на лету.

let audioCtx: AudioContext | null = null;

function ctx(): AudioContext | null {
  if (typeof window === "undefined") return null;
  if (!audioCtx) {
    const AC =
      window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return null;
    audioCtx = new AC();
  }
  if (audioCtx.state === "suspended") {
    audioCtx.resume().catch(() => {});
  }
  return audioCtx;
}

function tone(
  freq: number,
  duration: number,
  type: OscillatorType = "sine",
  volume = 0.07,
  delay = 0,
) {
  const c = ctx();
  if (!c) return;
  const t0 = c.currentTime + delay;
  const osc = c.createOscillator();
  const gain = c.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, t0);
  gain.gain.setValueAtTime(0.0001, t0);
  gain.gain.exponentialRampToValueAtTime(volume, t0 + 0.012);
  gain.gain.exponentialRampToValueAtTime(0.0001, t0 + duration);
  osc.connect(gain);
  gain.connect(c.destination);
  osc.start(t0);
  osc.stop(t0 + duration + 0.03);
}

/// Начало записи — восходящий «блип».
export function playRecordingStart() {
  tone(880, 0.14, "sine", 0.08);
  tone(1320, 0.16, "sine", 0.05, 0.06);
}

/// Конец записи — нисходящий «блип».
export function playRecordingStop() {
  tone(660, 0.12, "sine", 0.07);
  tone(440, 0.16, "sine", 0.05, 0.06);
}

/// Отправка сообщения — короткий «свип» вверх.
export function playSend() {
  tone(520, 0.09, "triangle", 0.07);
  tone(780, 0.11, "triangle", 0.06, 0.06);
}

/// Получение ответа — мягкий «поп».
export function playReceive() {
  tone(660, 0.1, "sine", 0.06);
  tone(990, 0.15, "sine", 0.05, 0.08);
}
