#!/usr/bin/env python3
"""Valida um PGN exportado com python-chess (DB-03).

python-chess é GPL: serve só como ferramenta externa de teste, nunca como
dependência do projeto (ADR-0004).

Uso: validate_pgn.py ARQUIVO.pgn PLIES FEN_FINAL
Sai com código 0 se o PGN reabre sem erros, com PLIES meios-lances e a
Posição final igual a FEN_FINAL (comparando só tabuleiro, vez e roque: a casa de en passant é omitida pelo python-chess quando não há captura).
"""
import sys

import chess.pgn


def main() -> int:
    path, plies, final_fen = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    with open(path, encoding="utf-8") as handle:
        game = chess.pgn.read_game(handle)
        if game is None:
            print("nenhuma partida no arquivo")
            return 1
        if chess.pgn.read_game(handle) is not None:
            print("o arquivo tem mais de uma partida")
            return 1
    if game.errors:
        print("erros ao ler o PGN:", game.errors)
        return 1
    moves = list(game.mainline_moves())
    if len(moves) != plies:
        print(f"esperava {plies} meios-lances, li {len(moves)}")
        return 1
    board = game.end().board()
    if board.fen().split()[:3] != final_fen.split()[:3]:
        print("Posição final difere:", board.fen(), "!=", final_fen)
        return 1
    for tag in ("Event", "Site", "Date", "Round", "White", "Black", "Result"):
        if tag not in game.headers:
            print("falta a tag", tag)
            return 1
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
