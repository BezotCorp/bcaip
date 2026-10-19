# Versioning

Le versioning BCAIP repose sur une séparation stricte entre :

- l'**intention déclarée** par les commits ;
- l'**état réel** du projet ;
- l'**identité fonctionnelle d'une version** ;
- l'**identité exacte d'un artefact construit** ;
- la **publication** ou le **déploiement** de cet artefact.

Aucun outil externe, forge, commit Git ou manifeste Cargo ne constitue à lui seul l'autorité sur le versioning BCAIP.

---

## Autorité

Le lifecycle manager est l'autorité centrale du cycle de vie des versions.

Les données structurées BCAIP constituent la source canonique.

Par exemple :

```text
data/
├── executables/
│   ├── <uuid>.ron
│   ├── index_by_name.ron
│   └── index_by_uuid.ron
│
└── libraries/
    ├── <uuid>.ron
    ├── index_by_name.ron
    └── index_by_uuid.ron
```

Les `Cargo.toml`, `Cargo.lock`, changelogs, tags Git, releases GitHub, packages crates.io et artefacts de déploiement sont des projections ou des sorties du lifecycle.

Ils ne sont pas la source d'autorité du modèle.

Le principe est :

```text
modèle BCAIP canonique
→ lifecycle manager
→ projections déterministes
```

Par exemple :

```text
Library.version
→ Cargo.toml
→ package
→ changelog
→ tag
→ release
→ publication éventuelle
```

---

## Version, artefact et transaction

Trois identités différentes doivent rester strictement séparées.

### Version

La version est l'identité fonctionnelle d'une release.

Exemple :

```text
1.4.0
```

Une version donnée représente un état donné.

Une même version ne doit jamais désigner deux contenus différents.

### SHA de l'exécutable

Le SHA-256 d'un exécutable est calculé **après le build**, directement sur le binaire produit.

Le nom doit rester explicite, par exemple :

```text
executable_sha256
```

Il ne s'agit pas :

- du SHA d'un commit Git ;
- du SHA d'une branche ;
- du SHA d'une release GitHub ;
- du SHA du repository.

Il représente uniquement l'empreinte exacte du binaire compilé.

Exemple :

```text
version = 1.4.0
executable_sha256 = abcdef...
```

Le SHA sert notamment à vérifier que l'artefact présent ou transféré correspond exactement au binaire attendu.

Il ne remplace jamais la version.

### Update ID

Un `update_id` identifie une tentative particulière de mise à jour.

Exemple :

```text
version = 1.4.0
executable_sha256 = abcdef...
update_id = <uuid-v7>
```

Une nouvelle tentative peut avoir un nouvel `update_id` tout en visant exactement la même version et le même artefact.

L'`update_id` n'est donc ni une version ni une identité d'artefact.

---

## Invariant version / artefact

Une version donnée doit correspondre à un artefact déterminé.

Si le système observe :

```text
version = 1.4.0
executable_sha256 = AAA
```

puis :

```text
version = 1.4.0
executable_sha256 = BBB
```

il existe une incohérence.

Ce cas ne doit pas être accepté comme une nouvelle variante normale de la même version.

La remise en conformité passe toujours par le mécanisme normal de mise à jour.

Il ne faut jamais corriger ce type d'écart par suppression ou remplacement brutal du binaire.

Principe :

```text
état désiré != état observé
→ convergence
→ procédure normale de mise à jour
```

---

## Indépendance vis-à-vis de Git

Le versioning BCAIP ne dépend pas de GitHub, Gitea ou d'un SHA de commit.

Une révision Git peut être enregistrée comme information de provenance, mais elle ne constitue pas l'identité fonctionnelle de la version.

Par exemple :

```text
version = 1.4.0
executable_sha256 = AAA
source_revision = abc123
```

Si le repository migre de GitHub vers Gitea, si l'historique est réécrit ou si la révision source d'origine n'est plus disponible :

```text
version = 1.4.0
executable_sha256 = AAA
```

reste un état parfaitement valide et compréhensible.

La provenance du source ne doit jamais être nécessaire au fonctionnement runtime.

---

## Libraries

Les libraries possèdent leur propre version SemVer.

Leur publication éventuelle sur crates.io ne détermine pas si elles doivent être versionnées.

Une library non publiée doit malgré tout évoluer :

```text
0.1.0
→ 0.1.1
→ 0.2.0
→ 1.0.0
```

selon son évolution réelle.

La version canonique appartient au modèle BCAIP de la library.

Le lifecycle manager peut ensuite matérialiser cette version dans son `Cargo.toml`.

Ainsi :

```text
data/libraries/<uuid>.ron
→ autorité

Cargo.toml
→ projection nécessaire à Cargo
```

Une publication sur crates.io constitue seulement une destination supplémentaire du lifecycle.

---

## Services et products

Les services et products sont également versionnés.

Leur cycle de vie diffère cependant de celui des libraries.

Exemple :

```text
changement
→ nouvelle version
→ build
→ executable_sha256
→ release
→ déploiement
→ validation
→ rollback éventuel
```

Les services et products déployés ne doivent jamais être modifiés directement hors du mécanisme de convergence une fois le bootstrap initial terminé.

Toute mise en conformité passe par le processus normal de mise à jour supervisée.

---

# Intention des commits

Les messages de commit expriment une **intention**, mais ne constituent jamais l'autorité sur l'impact réel d'un changement.

Conventions utilisées :

- `chore` : aucun changement fonctionnel annoncé ;
- `fix` : correction compatible ;
- `feat` : nouvelle fonctionnalité compatible ;
- `BREAKING CHANGE` : rupture de compatibilité annoncée.

Le système ne doit pas tenter de comprendre arbitrairement la sémantique métier d'une implémentation privée.

Il peut en revanche analyser les changements objectivement observables sur les surfaces publiques.

---

## Surface publique

L'analyse automatique doit se concentrer sur la surface réellement consommable.

Cela inclut notamment :

- API publique Rust ;
- CLI ;
- protocoles réseau ;
- formats d'échange ;
- formats de configuration ;
- schémas persistants ;
- autres contrats explicitement exposés.

Exemple CLI :

```text
avant :
--server-port

après :
argument supprimé
```

Résultat :

```text
breaking
```

Un consommateur valide auparavant ne peut plus appeler le programme de la même façon.

À l'inverse :

```text
ajout de --dry-run
```

étend la surface publique :

```text
feat
```

---

## Règles de classification

La règle générale est :

```text
surface publique cassée
→ breaking

surface publique étendue
→ feat

surface publique inchangée
→ intention du commit
```

Donc :

```text
surface inchangée + fix
→ patch

surface inchangée + feat
→ minor

surface inchangée + chore
→ aucun bump
```

Le système ne doit pas inventer une feature ou un fix qu'il ne peut pas prouver.

En particulier, un changement interne privé peut :

- corriger un bug ;
- ajouter une fonctionnalité ;
- changer un comportement métier ;

sans que l'AST permette à lui seul de connaître l'intention réelle.

Dans ce cas, le message de commit fournit l'information sémantique.

---

## Libraries et breaking changes

Pour les libraries, les ruptures de surface publique sont particulièrement importantes.

Une API publique cassée sous une version compatible constituerait une violation du contrat SemVer.

L'analyse automatique doit donc pouvoir détecter autant que possible :

- suppression d'un item public ;
- changement incompatible de signature ;
- changement incompatible de visibilité ;
- modification incompatible de trait ;
- suppression ou modification incompatible d'une variante ou structure publique ;
- autres changements de contrat public détectables.

Un changement interne entièrement contenu dans le workspace n'est pas automatiquement un breaking change externe.

La portée du symbole compte :

```text
privé
→ impact local

pub(crate)
→ impact au niveau du crate

pub
→ contrat potentiellement externe
```

Cette information aide à déterminer la portée du changement, mais ne suffit pas à deviner sa signification fonctionnelle.

---

# Validation des commits

À terme, l'intention déclarée par les commits sera contrôlée à plusieurs niveaux.

## Contrôle local

Un mécanisme `pre-commit` / `commit-msg` analysera les changements avant validation du commit.

Il devra détecter les incompatibilités réelles et refuser un commit dont le message ne correspond pas à son impact.

Exemple :

```text
chore: refactor library
```

alors qu'une API publique est cassée :

```text
→ commit refusé
→ BREAKING CHANGE requis
```

Ce contrôle local est destiné au retour immédiat au développeur.

---

## Contrôle CI

Le même principe sera vérifié en CI avant merge.

La CI constitue un second garde-fou contre :

- l'utilisation de `--no-verify` ;
- l'absence des hooks locaux ;
- une configuration locale incorrecte ;
- tout contournement accidentel du contrôle local.

Une incohérence doit donc être bloquée avant d'entrer normalement dans la branche destinée à la release.

Les mécanismes pre-commit et CI ne sont pas requis dans l'implémentation initiale du lifecycle manager.

Ils seront ajoutés ultérieurement.

---

# Autorité finale du lifecycle manager

Le lifecycle manager reste le dernier niveau de contrôle.

Il ne fait jamais confiance aveuglément au message de commit pour déterminer l'état à publier ou déployer.

Lors de la préparation d'une release, il compare l'état réellement publié précédemment avec l'état candidat courant.

Ainsi, même si les protections locales et CI ont été contournées ou ont échoué :

```text
commit annoncé : chore
impact réel objectivement détectable : breaking
```

le lifecycle manager retient :

```text
breaking
```

et non l'intention incorrectement déclarée.

En revanche, lorsqu'aucun impact plus fort ne peut être objectivement démontré, le lifecycle manager n'invente rien.

Exemple :

```text
commit annoncé : chore
surface publique inchangée
```

Résultat :

```text
aucun bump
```

Même si le changement interne aurait pu être décrit comme un `fix` ou un `feat`, le lifecycle manager ne tente pas de le deviner.

---

# Changelog

Le changelog est généré automatiquement par le lifecycle manager.

Il n'est pas une source d'autorité pour les versions.

Le lifecycle manager peut utiliser :

- l'historique depuis la dernière release ;
- les commits concernés ;
- les changements effectivement retenus dans la release ;
- leur classification.

Une représentation typique peut distinguer :

```text
Breaking Changes
Features
Fixes
Maintenance
```

Le changelog est un artefact de release.

Principe :

```text
état réel + intentions validées
→ nouvelle version
→ changelog
```

et non :

```text
changelog
→ découverte de la version
```

---

# Lifecycle de release

Le lifecycle manager orchestre l'ensemble du processus.

Conceptuellement :

```text
état canonique précédent
        ↓
état candidat
        ↓
analyse des changements
        ↓
calcul des nouvelles versions
        ↓
mise à jour du modèle BCAIP
        ↓
projection des versions dans Cargo.toml
        ↓
mise à jour Cargo.lock si nécessaire
        ↓
génération du changelog
        ↓
build
        ↓
calcul executable_sha256
        ↓
création de release / tag
        ↓
publication éventuelle
        ↓
déploiement éventuel
        ↓
validation / rollback / incidents
```

Les destinations dépendent du type de composant.

### Library

```text
version
→ Cargo.toml
→ package
→ release
→ crates.io éventuellement
```

### Service / product déployable

```text
version
→ build
→ executable_sha256
→ release
→ supervisor
→ convergence
→ validation
```

Le lifecycle manager peut utiliser GitHub aujourd'hui via `gh`.

GitHub reste cependant une destination.

Une migration future vers Gitea ou une autre forge ne doit pas modifier le modèle de versioning.

---

# Déploiement et convergence

Pour un executable, le lifecycle manager connaît l'état désiré :

```text
version
executable_sha256
```

Le supervisor fournit l'état réellement validé localement.

Exemple conforme :

```text
désiré :
version = 1.4.0
executable_sha256 = AAA

supervisor :
version = 1.4.0
executable_sha256 = AAA
```

Résultat :

```text
aucune action
```

Exemple version différente :

```text
désiré :
1.4.0 / AAA

supervisor :
1.3.0 / BBB
```

Résultat :

```text
mise à jour supervisée
```

Exemple même version mais mauvais artefact :

```text
désiré :
1.4.0 / AAA

supervisor :
1.4.0 / BBB
```

Résultat :

```text
état incohérent
→ remise en conformité via la procédure normale de mise à jour
```

Aucun remplacement brutal ne doit contourner le supervisor après le bootstrap initial.

---

# Principe général

```text
Commit
→ intention déclarée

Analyse structurelle
→ faits objectivement observables

Pre-commit / CI
→ vérification anticipée de l'intention

Lifecycle manager
→ autorité finale sur les versions et releases

Modèle BCAIP
→ source canonique

Cargo / Git / GitHub / crates.io / serveurs
→ projections et destinations
```

La règle fondamentale reste :

> Le système n'invente pas ce qu'il ne peut pas déterminer, mais il ne fait jamais confiance à une déclaration lorsqu'un état réel contradictoire peut être observé.
