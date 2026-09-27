// Speeds and ETAs from successive progress messages, averaged over the last few seconds so
// they don't jump around (RFD §5.3).

const WINDOW_MS = 3000;

type Sample = { t: number; bytes: number };

export class RateMeter {
  private samples: Sample[] = [];
  private first: Sample | null = null;

  /** Records `bytes` done at time `t` (milliseconds since the job started). */
  push(t: number, bytes: number): void {
    const sample = { t, bytes };
    this.first ??= sample;
    this.samples.push(sample);
    while (this.samples.length > 2 && t - this.samples[0].t > WINDOW_MS) {
      this.samples.shift();
    }
  }

  /** Bytes per second over the last few seconds; `null` until there are two samples. */
  current(): number | null {
    const [a, b] = [this.samples[0], this.samples[this.samples.length - 1]];
    if (!a || !b || b.t <= a.t) return null;
    return ((b.bytes - a.bytes) * 1000) / (b.t - a.t);
  }

  /** Bytes per second since the start; `null` until time has passed. */
  average(): number | null {
    const last = this.samples[this.samples.length - 1];
    if (!this.first || !last || last.t <= this.first.t) return null;
    return ((last.bytes - this.first.bytes) * 1000) / (last.t - this.first.t);
  }

  /** Milliseconds left for `remaining` bytes at the current speed; `null` when unknown. */
  eta(remaining: number): number | null {
    if (remaining <= 0) return 0;
    const speed = this.current();
    return speed && speed > 0 ? (remaining * 1000) / speed : null;
  }
}
