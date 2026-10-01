export interface ConnectionSample {
  bitrateMbps: number;
  fps: number;
  dropRate: number | null;
  targetFps: number;
}

export function formatConnectionHost(serial: string) {
  return serial.match(/^\[?([^\]]+)\]?:\d+$/)?.[1] ?? serial;
}

export function calculateConnectionSample(bytes: number, frames: number, droppedFrames: number, targetFps: number): ConnectionSample {
  const totalFrames = frames + droppedFrames;
  return {
    bitrateMbps: bytes * 8 / 1_000_000,
    fps: frames,
    dropRate: totalFrames ? droppedFrames / totalFrames * 100 : null,
    targetFps,
  };
}
