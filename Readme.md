# Just some random OpenGL playground

Run it with `cargo run`.

## Scenes

### `F1` Round Quads

Tons of randomly styled rounded rectangles, spinning faster the closer they are to the mouse.

### `F2` Blurring

An image of Gawr Gura being blurred.
The blur technique used is Kawase blur (two-pass diagonal gaussian blur) with color dithering.

Relevant articles:
- [Bandwidth-Efficient Rendering](<https://community.arm.com/cfs-file/__key/communityserver-blogs-components-weblogfiles/00-00-00-20-66/siggraph2015_2D00_mmg_2D00_marius_2D00_notes.pdf>)
  > **Note:** I used Kawase because it gave me better results than with normal XY blurring, and not because it was more bandwidth-efficient.
  > I was probably doing something wrong with XY blurring though, apparently I'm not supposed to get strong banding.
- [Removing Banding In Linelight](<https://pixelmager.github.io/linelight/banding.html>)

Keybinds:
- `K` - Toggle kawase blur
- `↑` - Increment blur kernel size
- `↓` - Decrement blur kernel size
