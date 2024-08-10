# Just some random OpenGL playground

Run it with `cargo run`.

## Scenes

### `F1` Round Quads

Tons of randomly styled rounded rectangles, spinning faster the closer they are to the mouse.

### `F2` Blurring

An image of Gawr Gura being blurred.
The blur technique used is sampled Gaussian blur, with recursive downsampling and color dithering.

Relevant articles:
- [Scale space implementation > The sampled Gaussian kernel][sampled-gaussian-kernel]
- [Removing Banding In Linelight][removing-banding-in-linelight]
- [Bandwidth-Efficient Rendering (Kawase blur)][bandwidth-efficient-rendering]
  > This is not a kawase blur, but the illustrations for recursively downsampling and upsampling in this document are nice and helpful.

Keybinds:
- `/` - Toggle diagonally sampled blur
- `D` - Toggle dithering
- `↑` - Increment blur kernel size
- `↓` - Decrement blur kernel size
- `→` - Increase blur radius
- `←` - Decrease blur radius
- `L` - Increase blur layers count
- `⇧L` - Decrease blur layers count

### `F3` Kawase Blur

An image of Gawr Gura being blurred.
The blur technique used is Kawase blur, with recursive downsampling and color dithering.

- [Removing Banding In Linelight][removing-banding-in-linelight]
- [An investigation of fast real-time GPU-based image blur algorithms][investigation-blur-algorithms]
- [Bandwidth-Efficient Rendering (Kawase blur)][bandwidth-efficient-rendering]

[sampled-gaussian-kernel]: https://en.wikipedia.org/wiki/Scale_space_implementation#The_sampled_Gaussian_kernel
[removing-banding-in-linelight]: https://pixelmager.github.io/linelight/banding.html
[bandwidth-efficient-rendering]: https://community.arm.com/cfs-file/__key/communityserver-blogs-components-weblogfiles/00-00-00-20-66/siggraph2015_2D00_mmg_2D00_marius_2D00_notes.pdf
[investigation-blur-algorithms]: https://www.intel.com/content/www/us/en/developer/articles/technical/an-investigation-of-fast-real-time-gpu-based-image-blur-algorithms.html

Keybinds:
- `D` - Toggle dithering
- `→` - Increase kawase distance
- `←` - Decrease kawase distance
