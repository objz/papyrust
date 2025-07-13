# Video FPS Fix Documentation

## Problem
Previously, videos in papyrust played at the wrong speed because they used the daemon's render FPS (controlled by the `--fps` flag) instead of respecting the video's original framerate. This caused:
- Videos to play too fast when their FPS was lower than the render FPS
- Videos to play too slow when their FPS was higher than the render FPS

## Solution
The VideoDecoder has been modified to:

1. **Extract Original Video FPS**: Automatically detects the video's original framerate from FFmpeg stream metadata
2. **Separate Timing**: Video frame timing is now independent of the render loop timing
3. **Accurate Playback**: Videos play at their original speed regardless of the `--fps` setting
4. **Smart Capping**: High FPS videos are gracefully capped at the render rate when needed

## How It Works

### FPS Detection
- Primary: Uses `stream.rate()` from FFmpeg
- Fallback: Uses `stream.time_base()` if rate is unavailable  
- Validation: Ensures FPS is reasonable (1-240 FPS range)
- Default: Falls back to 30 FPS if detection fails

### Timing Logic
- Tracks accumulated time since last video frame
- Only updates video frames when enough time has passed based on original video FPS
- Preserves timing accuracy even when render cycles don't align perfectly with video frame timing

### Behavior Examples
| Video FPS | Render FPS (`--fps`) | Result |
|-----------|---------------------|---------|
| 24        | 30                  | Plays at 24 FPS (correct speed) |
| 30        | 30                  | Plays at 30 FPS (correct speed) |
| 60        | 30                  | Plays at 30 FPS (capped by render rate) |
| 25        | 60                  | Plays at 25 FPS (correct speed) |

## Usage
The `--fps` parameter now specifically controls:
- Render loop frequency
- Shader animation speed
- Maximum video display rate

Videos automatically play at their correct speed without any additional configuration needed.

## Benefits
- ✅ Videos play at correct speed regardless of render FPS
- ✅ Maintains backward compatibility
- ✅ No configuration changes needed
- ✅ Robust FPS detection with fallbacks
- ✅ Efficient timing with minimal CPU overhead