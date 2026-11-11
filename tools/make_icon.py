from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

out = Path(__file__).resolve().parents[1] / "assets"
out.mkdir(parents=True, exist_ok=True)
size = 512
image = Image.new("RGBA", (size, size), (7, 17, 31, 255))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((28, 28, 484, 484), radius=112, fill=(13, 37, 67, 255), outline=(8, 124, 255, 255), width=14)
shield = [(256, 88), (398, 142), (384, 294), (324, 386), (256, 428), (188, 386), (128, 294), (114, 142)]
draw.polygon(shield, fill=(8, 74, 133, 255), outline=(32, 185, 244, 255))
try:
    font = ImageFont.truetype(r"C:\Windows\Fonts\segoeuib.ttf", 220)
except OSError:
    font = ImageFont.load_default()
bounds = draw.textbbox((0, 0), "W", font=font)
draw.text(((size-(bounds[2]-bounds[0]))/2, 135), "W", font=font, fill=(244, 248, 255, 255))
image.save(out / "WindscribeHub.png")
image.save(out / "WindscribeHub.ico", format="ICO", sizes=[(256,256),(128,128),(64,64),(48,48),(32,32),(16,16)])
