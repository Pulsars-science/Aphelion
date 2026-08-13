<h1 align="center">Aphelion</h1>

<p align="center">
  <strong>Un système solaire physiquement réel, dans lequel on peut mettre les mains.</strong>
</p>

<p align="center">
  <a href="#licence"><img alt="Licence : MIT OR Apache-2.0" src="https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.90+" src="https://img.shields.io/badge/rust-1.90%2B-orange.svg"></a>
  <a href="https://github.com/Pulsars-science/Aphelion/actions"><img alt="CI" src="https://github.com/Pulsars-science/Aphelion/actions/workflows/ci.yml/badge.svg"></a>
</p>

---

> 🇬🇧 The primary README is in English: **[README.md](README.md)**. Le code, les
> commentaires et la documentation technique sont en anglais, par convention
> open source. Cette page est là pour donner le contexte en français.

Aphelion affiche le système solaire en 3D et l'intègre avec une vraie gravité
newtonienne : chaque corps attire tous les autres, à chaque pas de temps. Rien
n'est sur rails. Les planètes sont là où elles sont parce que les équations les
y mettent.

Ensuite, le logiciel vous donne les constantes. Doublez `G`. Donnez à Jupiter
dix fois sa masse. Activez la correction relativiste et regardez le périhélie de
Mercure avancer. La simulation ne proteste pas — elle vous dit simplement, sans
tricher, à quel point elle conserve encore l'énergie.

> **Pourquoi « aphélie » ?** Le point de l'orbite le plus éloigné du Soleil —
> là où un corps est le plus lent, et d'où l'on voit toute sa trajectoire d'un
> seul coup d'œil.

## État

**v0.1 — alpha.** Le moteur physique est complet et testé ; le rendu et
l'interface fonctionnent ; le catalogue couvre le Soleil, les huit planètes, la
Lune et Pluton. L'API bougera avant la 1.0.

## Démarrage

```bash
git clone https://github.com/Pulsars-science/Aphelion.git
cd Aphelion
cargo run --release
```

`--release` compte : une compilation debug intègre environ dix fois plus
lentement.

Il faut Rust 1.90 ou plus récent et un GPU compatible Vulkan, Metal, DX12 ou
OpenGL — en pratique tout ce qui date de la dernière décennie, graphiques
intégrés compris.

### Commandes

| Entrée | Action |
|---|---|
| **Glisser** | Faire tourner la caméra |
| **Molette** | Zoom (multiplicatif : de la surface d'une lune jusqu'au-delà de Neptune) |
| **Espace** | Pause / reprise |
| **`[`** / **`]`** | Diviser / multiplier l'échelle de temps par deux |
| **`1`–`9`, `0`** | Cibler un corps |
| **`o`** | Afficher les orbites |
| **`f`** | Suivi de caméra |
| **`i`** | Changer d'intégrateur |
| **`r`** | Réinitialiser |
| **Échap** | Quitter |

Le reste est dans le panneau latéral.

## Ce qui est réellement simulé

C'est le point sur lequel il faut être précis, parce que « réaliste » ne coûte
rien à affirmer.

**La gravité est la vraie.** Chaque paire de corps s'attire selon la loi de
Newton, recalculée à chaque pas — aucune approximation à deux corps, aucune
orbite scriptée. C'est pour cela que le Soleil oscille visiblement autour du
barycentre, que Jupiter perturbe ses voisines, et que le système peut réellement
se disloquer si vous l'y poussez.

**L'intégration est symplectique.** Par défaut Verlet des vitesses ; la
composition d'ordre 4 de Yoshida est disponible pour les longues intégrations.
Les schémas symplectiques gardent l'erreur d'énergie *bornée et oscillante* au
lieu de la laisser dériver — c'est ce qui rend une intégration séculaire
crédible. L'interface affiche l'erreur relative en direct : vous savez toujours
quelle confiance accorder à ce que vous voyez.

À vérifier soi-même :

```bash
cargo run --release -p aphelion-data --example integrator_comparison
```

```text
intégrateur                  dt  éval.         pire        final       à 50 a
----------------------------------------------------------------------------
Velocity Verlet             1 j      2     8.265e-5     2.584e-6     4.874e-5
Yoshida 4                   1 j      6     1.135e-6     3.964e-8     8.769e-7
Runge-Kutta 4               1 j      4     2.618e-5     2.618e-5     1.309e-5
```

Regardez la dernière ligne. L'erreur de Runge–Kutta à 100 ans vaut exactement le
double de celle à 50 ans : une dérive à sens unique. Les schémas symplectiques
finissent bien en dessous de leur propre pire écart, parce qu'ils y reviennent.

**Les conditions initiales sont réelles.** Les corps partent des éléments
képlériens moyens J2000.0 du JPL, avec masses, rayons, périodes de rotation et
obliquités du JPL et de l'UAI. Vénus tourne à l'envers. Uranus est couchée sur
le côté. La Lune est en rotation synchrone.

**Relativité générale, en option.** La correction post-newtonienne d'ordre 1 due
à la masse dominante peut être activée. C'est elle qui rend compte des 43
secondes d'arc par siècle d'avance du périhélie de Mercure que Newton seul
n'explique pas.

### Ce qui n'est pas simulé

Pour être net sur les limites :

- **Ce n'est pas une éphéméride.** Les éléments moyens placent chaque planète à
  une fraction de degré de sa position vraie à J2000, pas à la seconde d'arc.
  Pour un vrai travail d'observation, il faut JPL DE440 — c'est sur la feuille
  de route.
- **Pas de gravité non sphérique.** Les corps sont des masses ponctuelles :
  ni aplatissement `J2`, ni forces de marée, ni pression de radiation.
- **Pas de collisions.** Les corps se traversent. Un adoucissement de Plummer
  est proposé pour éviter qu'une rencontre rapprochée n'éjecte quelque chose.
- **Textures procédurales.** Les planètes sont des couleurs unies avec une
  lumière de bord et un léger bandage, pas des surfaces photographiques.

## Les molettes

| Réglage | Effet |
|---|---|
| **Gravité ×G** | Met à l'échelle la constante gravitationnelle. Tout ce qui était en orbite circulaire se retrouve instantanément à la mauvaise vitesse — c'est tout l'intérêt. |
| **Toutes les masses** | Met à l'échelle toutes les masses d'un coup. |
| **Masse individuelle** | Par corps, de 1/100 à 100× la valeur réelle. |
| **Adoucissement** | Longueur de Plummer ; plafonne la force lors d'une rencontre rapprochée. |
| **Relativité** | La correction 1PN décrite plus haut. |
| **Intégrateur** | Euler, Verlet, Yoshida 4 ou RK4 — l'indicateur d'énergie réagit. |
| **Pas / orbite** | Précision contre coût, linéairement. |
| **Taille des corps ×** | Affichage uniquement. À l'échelle vraie, la Terre fait un cinquième de pixel dès qu'on voit son orbite. |

## Architecture

```
aphelion-core ──▶ aphelion-data ──▶ aphelion-gfx ──▶ aphelion-app
   physique         le système          rendu           fenêtre
```

| Crate | Rôle | Dépend de |
|---|---|---|
| [`aphelion-core`](crates/aphelion-core) | Forces N-corps, intégrateurs, éléments képlériens, époques. Aucun graphisme. | `glam` |
| [`aphelion-data`](crates/aphelion-data) | Le système solaire à J2000.0, sources citées. | core |
| [`aphelion-gfx`](crates/aphelion-gfx) | Rendu wgpu, caméra à l'échelle astronomique. Aucun fenêtrage. | core, `wgpu` |
| [`aphelion-app`](crates/aphelion-app) | Fenêtre winit, panneau egui, entrées. | tout |

`aphelion-core` s'utilise seul, sans affichage, comme bibliothèque N-corps.

### Deux problèmes qui valent la lecture

Les deux sont documentés en détail dans le code, parce que ce sont exactement le
genre de choses qui ruinent silencieusement un moteur d'astronomie :

- **Précision.** Neptune est à 4,5 × 10¹² m, et un `f32` a sept chiffres
  significatifs — un rendu naïf quantifie les positions par pas plus grands que
  la planète elle-même. Aphelion reste en `f64` et convertit *relativement à la
  caméra*, en unités astronomiques, au dernier moment possible. Voir
  [`aphelion-gfx/src/camera.rs`](crates/aphelion-gfx/src/camera.rs).
- **Profondeur.** Afficher une lune à 1000 km et une planète à 30 UA dans la
  même image, c'est un rapport near:far de 10¹². Une projection en Z inversé
  avec plan lointain à l'infini donne une précision relative quasi uniforme à
  toutes les échelles, et plus rien à découper. Même fichier.

## Développement

```bash
cargo test --workspace        # ~48 tests, dont des intégrations séculaires
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo doc --open -p aphelion-core
```

Les tests vérifient la physique, pas seulement la tuyauterie : la troisième loi
de Kepler contre les périodes publiées, la conservation de l'énergie et du moment
cinétique, la comparaison symplectique / RK4, la réversibilité temporelle, et la
survie du système interne sur un siècle.

## Feuille de route

Voir la section [Roadmap du README anglais](README.md#roadmap) : éphémérides
DE440, pas de temps adaptatif, arbre de Barnes–Hut, collisions, textures,
champ d'étoiles, build WebAssembly, scénarios sauvegardables.

## Contribuer

Les contributions sont bienvenues, de la correction de faute de frappe au
nouvel intégrateur. Commencez par [CONTRIBUTING.md](CONTRIBUTING.md) : modèle de
branches, convention de commits, et ce que la relecture regarde.

Le code, les commentaires, les messages de commit et les issues sont en anglais.
Les échanges dans les Discussions peuvent être en français.

Toute personne participant au projet est tenue de respecter le
[Code de conduite](CODE_OF_CONDUCT.md).

## Licence

Double licence, au choix :

- Licence Apache version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- Licence MIT ([LICENSE-MIT](LICENSE-MIT))

C'est l'arrangement standard de l'écosystème Rust.

### Attribution des données

Les éléments orbitaux et paramètres physiques proviennent de sources du domaine
public : [JPL Solar System Dynamics](https://ssd.jpl.nasa.gov/) et les rapports
du groupe de travail de l'UAI sur les coordonnées cartographiques et les
éléments de rotation. Les citations précises sont dans
[`crates/aphelion-data/src/solar_system.rs`](crates/aphelion-data/src/solar_system.rs).

---

<p align="center">
  Développé par <a href="https://github.com/Pulsars-science">Pulsars Science</a>.
</p>
