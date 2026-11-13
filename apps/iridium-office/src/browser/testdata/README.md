# Test images

`tiny.jpg` is a 2x2 JPEG used by the frame-decoding test.

It was made with the `image` crate, not by a browser, so it does not depend on
a browser being installed:

```rust
let mut img = image::RgbImage::new(2, 2);
img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
img.put_pixel(1, 0, image::Rgb([0, 255, 0]));
img.put_pixel(0, 1, image::Rgb([0, 0, 255]));
img.put_pixel(1, 1, image::Rgb([255, 255, 0]));
image::DynamicImage::ImageRgb8(img)
    .write_to(&mut out, image::ImageFormat::Jpeg)?;
```
