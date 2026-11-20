#!/usr/bin/env python3
"""Gera o ícone PROVISÓRIO do Chess Studio em crates/chess-studio/icons/ (requer Pillow).

Cavalo U+265E da Noto Sans Symbols 2 (SIL OFL 1.1, já em assets/fonts/) sobre um quadrado
arredondado cor de nogueira. Uso: python3 scripts/gerar-icone.py

- PNGs (macOS .icns e ícones do Linux): margem da grade de ícones da Apple (100 px em 1024).
- icon.ico (Windows): quase sem margem, que é o costume no Explorer e no Menu Iniciar.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

RAIZ = Path(__file__).resolve().parent.parent
FONTE = RAIZ / "crates/chess-studio/assets/fonts/NotoSansSymbols2-Regular.ttf"
SAIDA = RAIZ / "crates/chess-studio/icons"
CAVALO = "♞"
TOPO, BASE = (0x5A, 0x3D, 0x2B), (0x2E, 0x1F, 0x15)  # nogueira, claro -> escuro
PECA = (0xF4, 0xE9, 0xD8, 255)
LADO = 1024


def desenhar(margem: int) -> Image.Image:
    corpo = LADO - 2 * margem
    # Fundo: degradê vertical recortado por um quadrado arredondado.
    fundo = Image.new("RGBA", (LADO, LADO))
    px = ImageDraw.Draw(fundo)
    for y in range(LADO):
        t = min(max((y - margem) / corpo, 0.0), 1.0)
        cor = tuple(round(a + (b - a) * t) for a, b in zip(TOPO, BASE))
        px.line([(0, y), (LADO, y)], fill=(*cor, 255))
    mascara = Image.new("L", (LADO, LADO), 0)
    ImageDraw.Draw(mascara).rounded_rectangle(
        [margem, margem, LADO - margem - 1, LADO - margem - 1], radius=round(corpo * 0.225), fill=255
    )
    img = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0))
    img.paste(fundo, (0, 0), mascara)

    # Cavalo centrado pela caixa do glifo, com ~64% da altura do corpo.
    fonte = ImageFont.truetype(str(FONTE), 100)
    x0, y0, x1, y1 = fonte.getbbox(CAVALO)
    fonte = ImageFont.truetype(str(FONTE), round(100 * corpo * 0.64 / (y1 - y0)))
    x0, y0, x1, y1 = fonte.getbbox(CAVALO)
    pos = ((LADO - (x1 - x0)) / 2 - x0, (LADO - (y1 - y0)) / 2 - y0)

    sombra = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0))
    ImageDraw.Draw(sombra).text((pos[0], pos[1] + corpo * 0.015), CAVALO, font=fonte, fill=(0, 0, 0, 110))
    sombra = sombra.filter(ImageFilter.GaussianBlur(corpo * 0.02))
    img = Image.alpha_composite(img, Image.composite(sombra, Image.new("RGBA", sombra.size), mascara))
    ImageDraw.Draw(img).text(pos, CAVALO, font=fonte, fill=PECA)
    return img


def main() -> None:
    SAIDA.mkdir(parents=True, exist_ok=True)
    mac = desenhar(margem=100)
    for nome, lado in [
        ("32x32.png", 32),
        ("64x64.png", 64),
        ("128x128.png", 128),
        ("128x128@2x.png", 256),
        ("256x256.png", 256),
        ("512x512.png", 512),
        ("512x512@2x.png", 1024),
    ]:
        mac.resize((lado, lado), Image.LANCZOS).save(SAIDA / nome, optimize=True)
    win = desenhar(margem=16)
    win.save(SAIDA / "icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    print(f"gerado: {SAIDA.relative_to(RAIZ)}/ (PNGs e icon.ico)")


if __name__ == "__main__":
    main()
