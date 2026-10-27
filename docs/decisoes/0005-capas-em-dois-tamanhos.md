# 0005 — Capas em dois tamanhos

**Status:** aceita (2026-10-26)

## Contexto

Uma imagem na tela ocupa memória **decodificada**, não o tamanho do arquivo:

```
640 × 640 pixels × 4 bytes (RGBA) ≈ 1,6 MB por capa
```

Uma lista de 50 resultados com capas 640px gastaria **~80 MB só em imagens**, mais da
metade da meta de 150 MB, para desenhar miniaturas de 44px.

## Decisão

O modelo `Artwork` (`artwork.rs`) tem dois campos:

- `thumb` (~160px, cerca de 100 KB decodificada): linhas, grade da biblioteca e fundo ambiente;
- `full` (~544px): só a capa grande do "tocando agora".

Cada serviço oferece tamanhos de um jeito diferente, e cada um tem sua função com teste:

| Serviço | Como obtemos os dois tamanhos |
|---|---|
| Spotify | Já devolve uma lista de imagens 640/300/64; pegamos a de 300 e a de 640 |
| YouTube Music | URLs `lh3.googleusercontent.com/...=w120-h120` aceitam **qualquer tamanho** trocando o sufixo |
| SoundCloud | `...-large.jpg` (100px) também existe em `-t200x200` e `-t500x500` |

## Detalhe esperto

O fundo ambiente usa a **miniatura**. Com 70px de desfoque, ninguém percebe a resolução,
e o custo de memória é cerca de 25x menor.
