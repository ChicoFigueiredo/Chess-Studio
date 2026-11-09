# B31 — interface duplicada no 2D técnico (issue #142)

Capturas: `antes/` (main) e `depois/` (correção), geradas com
`chess-studio --screenshot <dir> --panel --quality ultra` (Ultra + bloom, as 3 Apresentações).

Nota honesta: o modo `--screenshot` troca de Apresentação sem teclado e sem tooltip, e não
reproduziu o resíduo visual da homologação (as duas capturas do 2D técnico saem limpas).
A correção ataca a causa provável (HDR só na câmera 3D, ver PR) e é garantida por teste
(`one_ui_camera_and_uniform_camera_setup_after_cycling_presentations`); a confirmação visual
com Tab + tooltip fica para a homologação do usuário.
