import numpy as np

from compose import OUT, SR, Mix, master, score, true_peak, write
from sfx import sfx


def main():
    mix = Mix()
    kicks = score(mix)
    x, lufs, stems = master(mix, kicks)
    write(OUT / "music.wav", x)
    print(f"music: {len(x) / SR:.3f} s, {lufs:.2f} LUFS, true peak {20 * np.log10(true_peak(x)):.2f} dBTP")
    print("stems (dB RMS):", {k: round(v, 1) for k, v in stems.items()})
    # Shares compose's generator, so runs after the music.
    sfx()


if __name__ == "__main__":
    main()
