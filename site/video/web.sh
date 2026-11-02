#!/usr/bin/env bash
# Builds web clips, posters and the launch film.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
rec="$here/out/rec"
clips="$here/../public/clips"
film="$here/../public/film"
mkdir -p "$clips" "$film"

# name source start end
cuts=(
  "doc doc 3.0 10.6"
  "table table 16.0 23.6"
  "show show-play 2.6 9.6"
  "pdf pdf 8.8 16.3"
  "mail mail 6.9 14.9"
  "tree tree 9.5 16.8"
  "mark mark 5.4 13.4"
)

fade=0.2
size="scale=1280:720:flags=lanczos,fps=30,format=yuv420p"

for cut in "${cuts[@]}"; do
  read -r name src start end <<<"$cut"
  len=$(echo "$end - $start" | bc)
  at=$(echo "$len - $fade" | bc)
  one=$(echo "$start + 0.05" | bc)
  frames=$(echo "$len * 30 / 1" | bc)
  # Last 0.2 s fades into the first frame.
  graph="[0:v]trim=start=$start:end=$end,setpts=PTS-STARTPTS,$size[a];
[0:v]trim=start=$start:end=$one,setpts=PTS-STARTPTS,$size,trim=end_frame=1,tpad=stop_mode=clone:stop_duration=$fade[b];
[a][b]xfade=transition=fade:duration=$fade:offset=$at[v]"
  ffmpeg -v error -y -i "$rec/$src.mp4" -filter_complex "$graph" -map "[v]" -frames:v "$frames" -an \
    -c:v libx264 -profile:v high -preset slow -crf 24 -pix_fmt yuv420p \
    -movflags +faststart "$clips/$name.mp4"
  ffmpeg -v error -y -i "$clips/$name.mp4" -an \
    -c:v libvpx-vp9 -crf 36 -b:v 0 -row-mt 1 -deadline good -cpu-used 2 \
    "$clips/$name.webm"
  ffmpeg -v error -y -i "$clips/$name.mp4" -frames:v 1 -q:v 4 "$clips/$name.jpg"
done

src="$here/out/ElyOffice-launch.mp4"
ffmpeg -v error -y -i "$src" -vf "scale=1920:1080:flags=lanczos,format=yuv420p" \
  -c:v libx264 -profile:v high -preset slow -crf 23 \
  -c:a aac -b:a 160k -movflags +faststart "$film/ely-office.mp4"
ffmpeg -v error -y -ss 17 -i "$src" -frames:v 1 -q:v 3 "$film/poster.jpg"

ls -l "$clips" "$film"
