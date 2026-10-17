#!/usr/bin/env bash
# Generates the GitHub social preview card (1280x640) for Orquestador.
# Deterministic layout: every text element is rendered as its own label image
# and composited at an explicit top-left offset, so positions are exact.
set -euo pipefail

FB=/usr/share/fonts/julietaula-montserrat-fonts/Montserrat-Bold.otf
OUTDIR=/tmp/opencode/og
mkdir -p "$OUTDIR"
OUT="$OUTDIR/social-preview.png"

# Brand palette (frontend/src/styles.css)
BG=#0b0e13
SURFACE=#12161e
BORDER=#242b38
TEXT=#e9edf5
MUTED=#93a0b6
BRAND=#34d07f
SECONDARY=#6aa6ff
ACCENT=#29d3c4
WARNING=#ffb02e
HEADERBAR=#1e2531
LINEFILL=#2b3444
LABELFILL=#3a4354

W=1280
H=640

# ---- geometry (locked design) -------------------------------------------
PAD=72
TX=$((PAD + 42))          # text column left edge (right of the accent bar)
BAR_X=$PAD                # accent bar
BAR_W=12

KICK_Y=$PAD                       # kicker top
TITLE_Y=$((PAD + 42))             # title top
SUB1_Y=$((PAD + 180))             # subtitle line 1
SUB2_Y=$((PAD + 222))             # subtitle line 2
TAG_Y=$((PAD + 288))              # tagline
BAR_TOP=$PAD
BAR_BOT=$((TAG_Y + 41))

# panes
PG_X=$((PAD + 718))               # 790
PG_X2=1208
PG_Y=$PAD
PG_Y2=$((PG_Y + 398))             # 470
GAP=16
PANE_W=$(( (PG_X2 - PG_X - GAP) / 2 ))   # 201
PANE_H=$(( (PG_Y2 - PG_Y - GAP) / 2 ))   # 191

DIV_Y=505
FOOT_Y=$((H - PAD - 29))          # 539
DOT_Y=$((FOOT_Y + 15))

# ---- text labels ---------------------------------------------------------
label() { # pointsize color text -> file
  magick -background none -font "$FB" -pointsize "$1" -fill "$2" \
    label:"$3" "$OUTDIR/l-$4.png"
  magick identify -format "$4 %wx%h\n" "$OUTDIR/l-$4.png"
}

label 22 "$BRAND"  "OPENCODE  ·  GENTLE-AI"                        kick
label 96 "$TEXT"   "Orquestador"                                   title
label 28 "$MUTED"  "The desktop control tower for OpenCode"        sub1
label 28 "$MUTED"  "and gentle-ai orchestrator sub-agents."        sub2
label 32 "$SECONDARY" "Every sub-agent in its own live pane."      tag
label 22 "$MUTED"  "github.com/alecerca/ORCHESTRATOR"              foot

# ---- base plate: background + shapes ------------------------------------
# pane internal furniture as a draw string, one pane at (px,py)
pane_draw() { # px py stroke color
  local px=$1 py=$2 stroke=$3 dot=$4
  local sw=2
  [ "$stroke" != "$BORDER" ] && sw=2
  printf "%s" "fill $SURFACE stroke $stroke stroke-width $sw roundrectangle $px,$py $((px+PANE_W)),$((py+PANE_H)) 12,12 "
  printf "%s" "fill $HEADERBAR stroke none roundrectangle $((px+14)),$((py+14)) $((px+PANE_W-14)),$((py+40)) 6,6 "
  printf "%s" "fill $dot stroke none circle $((px+30)),$((py+27)) $((px+35)),$((py+27)) "
  printf "%s" "fill $LABELFILL stroke none roundrectangle $((px+46)),$((py+22)) $((px+140)),$((py+32)) 3,3 "
  local ly
  for ly in 58 78 98 118 138 158; do
    local lw=166
    case $ly in 78) lw=144;; 98) lw=114;; 118) lw=156;; 138) lw=131;; 158) lw=96;; esac
    printf "%s" "fill $LINEFILL stroke none roundrectangle $((px+14)),$((py+ly)) $((px+14+lw)),$((py+ly+8)) 4,4 "
  done
}

P1X=$PG_X;               P1Y=$PG_Y
P2X=$((PG_X + PANE_W + GAP)); P2Y=$PG_Y
P3X=$PG_X;               P3Y=$((PG_Y + PANE_H + GAP))
P4X=$((PG_X + PANE_W + GAP)); P4Y=$((PG_Y + PANE_H + GAP))

{
  printf "%s" "fill $BG stroke none rectangle 0,0 $W,$H "
  printf "%s" "fill $BRAND stroke none roundrectangle $BAR_X,$BAR_TOP $((BAR_X+BAR_W)),$BAR_BOT 6,6 "
  printf "%s" "stroke $BORDER stroke-width 1 fill none line $PAD,$DIV_Y $((W-PAD)),$DIV_Y "
  printf "%s" "fill $BRAND stroke none circle $((PAD+8)),$DOT_Y $((PAD+13)),$DOT_Y "
  pane_draw "$P1X" "$P1Y" "$BRAND"   "$BRAND"
  pane_draw "$P2X" "$P2Y" "$BORDER"  "$SECONDARY"
  pane_draw "$P3X" "$P3Y" "$BORDER"  "$WARNING"
  pane_draw "$P4X" "$P4Y" "$BORDER"  "$ACCENT"
} > "$OUTDIR/draw.txt"

magick -size ${W}x${H} xc:"$BG" -draw "@$OUTDIR/draw.txt" "$OUTDIR/base.png"

magick "$OUTDIR/base.png" \
  "$OUTDIR/l-kick.png"  -geometry +${TX}+${KICK_Y}  -composite \
  "$OUTDIR/l-title.png" -geometry +${TX}+${TITLE_Y} -composite \
  "$OUTDIR/l-sub1.png"  -geometry +$((TX+2))+${SUB1_Y} -composite \
  "$OUTDIR/l-sub2.png"  -geometry +$((TX+2))+${SUB2_Y} -composite \
  "$OUTDIR/l-tag.png"   -geometry +$((TX+2))+${TAG_Y}  -composite \
  "$OUTDIR/l-foot.png"  -geometry +$((PAD+26))+${FOOT_Y} -composite \
  "$OUT"

echo "--- result ---"
magick identify -format "size=%wx%h colors=%k bytes=%b\n" "$OUT"
magick identify -format "ink-bounds(trim)=%wx%h%O\n" <(magick "$OUT" -bordercolor "$BG" -border 1 -trim png:-) 2>/dev/null || true
echo "file: $OUT"
