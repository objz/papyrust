# Testing the Wayland Layer-Shell Rotation Fix

## What was changed

This fix addresses the issue where Papyrust's wallpaper does not cover the full screen on rotated monitors, leaving black borders.

### Key Changes Made:

1. **Fixed buffer transform**: Changed from using output transform to `Normal` (line 849)
2. **Fixed layer surface sizing**: Added anchors to all edges and set size to (0,0) (lines 870-871)
3. **Added configure handling**: Now captures compositor-provided logical dimensions (lines 763, 1084-1090)
4. **Fixed renderer**: Removed coordinate swapping based on transform (line 230)
5. **Added EGL window resizing**: Dynamically resize based on configure events (line 1087)

## How to test

### Test Case 1: Rotated Monitor (Main fix)
1. Set up a monitor in portrait mode (90° or 270° rotation)
2. Run papyrust-daemon on that monitor
3. **Expected**: Wallpaper should now cover the entire screen with no black borders
4. **Previously**: There would be black borders on top/right edges

### Test Case 2: Normal Monitor (Regression test)
1. Use papyrust-daemon on a standard landscape monitor
2. **Expected**: Behavior should be unchanged - full coverage as before
3. **Risk**: Ensure we didn't break normal monitor support

### Test Case 3: Runtime Rotation (Dynamic test)
1. Start papyrust-daemon on a monitor
2. Rotate the monitor while papyrust is running (using display settings)
3. **Expected**: Wallpaper should adjust without gaps or restart needed
4. **Previously**: Would likely show black borders after rotation

### Test Case 4: Multiple Monitors with Mixed Orientations
1. Set up multiple monitors with different orientations (one portrait, one landscape)
2. Run papyrust-daemon
3. **Expected**: Each monitor should have proper coverage regardless of orientation

## Technical Verification

### Check the fix is working:
- Monitor Wayland protocol messages to see configure events being handled
- Verify EGL window resize calls are happening with correct logical dimensions
- Confirm buffer transform is set to Normal instead of the output transform

### Debug logging suggestions:
Add debug prints to verify:
- Configure events received: `(width, height)` values
- EGL window resize calls: `resize(new_w, new_h, 0, 0)`
- Final dimensions used in renderer: `output_width × output_height`

## Expected behavior change

**Before fix**: 
- Buffer transform = output transform (90°, 270°, etc.)
- Layer surface size = physical mode size (1920×1080 on rotated display)
- Black borders visible on rotated monitors

**After fix**:
- Buffer transform = Normal (always)
- Layer surface size = (0,0) with full anchoring (let compositor decide)
- EGL window resized to logical dimensions from configure events
- Full coverage on all monitor orientations