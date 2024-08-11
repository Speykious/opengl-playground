# OpenGL playground

This is my personal playground to learn and experiment with low-level graphics programming stuff in OpenGL.

It runs with OpenGL >=3.3 on Windows, Linux and MacOS.

You can just run it with `cargo run`.

## Scenes

### `F1` Round Quads

<div align="center">
  <figure align="center">
    <img  align="center" src="https://fs.speykious.dev/opengl-squares/round-rects.png" alt="Round rects" />
    <figcaption align="center"><i>Round quads scene</i></figcaption>
  </figure>
</div>

&nbsp;

Tons of randomly styled rounded rectangles, spinning faster the closer they are to the mouse.

### `F2` Blurring

<div align="center">
  <figure align="center">
    <img  align="center" src="https://fs.speykious.dev/opengl-squares/blur-gaussian-k5r2l4.png" alt="Blurring scene" />
    <figcaption align="center"><i>Gaussian blur with kernel 5, radius 2 and 4 downsampling layers (no color dithering)</i></figcaption>
  </figure>
</div>

&nbsp;

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

<div align="center">
  <figure align="center">
    <img  align="center" src="https://fs.speykious.dev/opengl-squares/blur-kawase-r2l4.png" alt="Kawase Blur scene" />
    <figcaption align="center"><i>Kawase-derived Dual-filter blur with radius 2 and 4 downsampling layers (no color dithering)</i></figcaption>
  </figure>
</div>

&nbsp;

An image of Gawr Gura being blurred.
The blur technique used is Dual Filtering, derived from the Kawase blur, with recursive downsampling and color dithering.

Relevant articles:
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
- `L` - Increase blur layers count
- `⇧L` - Decrease blur layers count
