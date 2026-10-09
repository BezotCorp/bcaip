# BCAIP — Vision Originelle (Par Rémi)

Ce document fige la vision brute et conceptuelle de BCAIP, indépendamment de toute source d'alimentation (VS Code ou autre), de toute interface et de tout découpage logiciel préconçu.

BCAIP n'a pas vocation à être une simple IA qui utilise des outils, ni un modèle géant auquel on demande de tout connaître. C'est un **système coopératif guidé par l'intention**, dans lequel les intelligences neuronales, les mécanismes symboliques et les mémoires spécialisées travaillent ensemble.

L'intelligence du système ne repose pas uniquement sur les capacités individuelles de ses composants, mais aussi sur leur coopération.

---

## 1. L'Intention Pilote Tout

L'intention pilote l'intégralité du système, y compris pour le développement.

L'utilisateur exprime ce qu'il veut obtenir. Le système détermine comment réaliser cette intention et lui fournit le résultat attendu.

Il en va de même pour les IA internes : elles peuvent exprimer une intention brute sans avoir besoin de connaître les techniques, les commandes ou les outils sous-jacents.

**Exemples :**

- Une IA dit : « J'aimerais supprimer ce fichier. » Elle n'a pas besoin de connaître `rm`, ses options ou le fonctionnement du système de fichiers. Elle exprime son intention et reçoit, si l'opération réussit, un résultat simple : « Fichier supprimé. »
- Une IA souhaite créer un fichier à un endroit donné avec un contenu précis. Le système effectue l'opération et confirme son résultat. Il n'est pas nécessaire de réinjecter le contenu du fichier dans le contexte de l'IA si elle le connaît déjà.
- Une IA veut modifier la visibilité d'une fonction Rust. Elle n'a pas nécessairement besoin de manipuler elle-même les octets du fichier : elle peut exprimer la transformation souhaitée.

Le système ne doit pas confondre intention et procédure.

**Le contexte transmis à l'IA doit contenir les informations utiles à la poursuite de son intention, sans répéter inutilement les données déjà connues.**

Cela permet notamment de réduire la consommation de tokens, sans priver les IA des informations nécessaires lorsqu'une opération échoue ou produit un résultat inattendu.

## 2. Le Cerveau Minimal et la Délégation

Le modèle neuronal n'a pas vocation à tout mémoriser, ni à reproduire des compétences déterministes déjà maîtrisées par d'autres composants.

Le système repose sur la coexistence de deux espaces complémentaires :

- **L'espace neuronal**, capable d'interprétation, de généralisation, de raisonnement incertain et d'adaptation.
- **L'espace symbolique**, capable d'exécuter des opérations explicites, déterministes et vérifiables.

Chacun son travail.

Si une IA doit effectuer un calcul, elle peut demander à une calculatrice plutôt que de calculer elle-même. Si elle doit comprendre un projet Rust, elle peut interroger un analyseur de code plutôt que de reconstruire toutes les relations depuis des fichiers bruts.

La délégation n'est pas un aveu d'échec : **c'est un mécanisme fondamental de l'intelligence du système**.

Une IA peut ne maîtriser qu'une compétence très précise. Elle n'a pas besoin de connaître toutes les autres compétences disponibles.

L'objectif est de permettre à des cerveaux neuronaux plus petits, plus spécialisés et plus plastiques de participer à un système dont les capacités globales dépassent celles de chaque composant isolé.

## 3. Le Code sous forme de Représentation Sémantique Persistante

Le code et son contexte doivent pouvoir être conservés sous une forme sémantique durable, notamment à travers des AST (_Abstract Syntax Trees_) persistants et régulièrement actualisés.

L'objectif n'est pas de forcer une IA à relire continuellement les mêmes fichiers pour retrouver leur structure.

**Exemple :**

Une IA demande : « Si je touche à `fn pipo`, qu'est-ce que je risque de casser ? »

Le système consulte sa représentation du projet, ses relations entre symboles et les informations disponibles sur les dépendances, puis lui communique les impacts identifiés.

L'IA peut ensuite exprimer son intention :

« Je veux transformer cette fonction publique en fonction privée. »

Cette modification peut être déléguée à une compétence spécialisée, y compris une IA modificatrice.

Le système applique la transformation, vérifie son résultat et actualise sa représentation persistante en arrière-plan.

L'AST constitue une base structurale importante, mais l'analyse complète des impacts peut également nécessiter des informations sémantiques supplémentaires : résolution des symboles, types, références et dépendances.

L'objectif reste le même : **le projet doit être compris et représenté par le système, pas reconstruit intégralement dans le contexte neuronal à chaque demande.**

## 4. L'IA Fondamentalement Humaine — La Fiabilité par le Doute

L'IA idéale n'est pas omnipotente.

Elle peut ignorer quelque chose, mal comprendre une intention, se tromper dans son raisonnement ou produire une réponse incorrecte.

La fiabilité recherchée ne repose donc pas sur l'idée qu'une IA ne commettra jamais d'erreur, mais sur sa capacité à coopérer avec un système qui peut les détecter, les examiner et les corriger.

Une IA doit pouvoir :

- Reconnaître qu'elle ne sait pas.
- Demander des explications lorsqu'une intention lui échappe.
- Déléguer une question à une compétence plus appropriée.
- Accepter qu'un autre composant contredise sa conclusion.
- Réévaluer une réponse à partir d'informations nouvelles ou d'une contradiction identifiée.

**Une affirmation confiante n'est pas nécessairement une affirmation correcte.**

Le système ne doit pas donner raison à l'utilisateur par complaisance, ni considérer ses propres réponses comme des vérités simplement parce qu'elles ont déjà été formulées.

L'utilisateur peut se tromper. L'IA aussi.

Les outils, comme `rust-analyzer` ou `git`, sont compris et exploités par le système pour fournir des éléments vérifiables plutôt que d'exiger des IA qu'elles devinent leurs résultats.

La capacité à douter, à vérifier et à demander de l'aide constitue une propriété essentielle de BCAIP.

## 5. La Coopération et la Sécurité Autonome

La coopération ne se limite pas à une IA principale accompagnée de quelques outils.

BCAIP peut faire intervenir des composants symboliques, des IA de compréhension du langage, des analystes, des compétences spécialisées et des mécanismes de mémoire.

Chaque composant peut comprendre une partie seulement du problème et déléguer le reste.

**Le système symbolique peut lui-même demander de l'aide.**

Lorsqu'il reçoit une intention qu'il ne sait pas interpréter, il peut solliciter une compétence neuronale pour en clarifier le sens.

Le routage peut être symbolique lorsqu'une règle suffit, ou partiellement neuronal lorsqu'une interprétation est nécessaire.

**Exemple de sécurité :**

Une IA exprime l'intention de supprimer un fichier.

Le système peut analyser les conséquences de cette suppression, consulter les dépendances du fichier ou déléguer une analyse de risque.

Une IA spécialisée peut alors signaler :

« Attention, si ce fichier disparaît, l'ordinateur risque de ne plus démarrer. »

La sécurité n'a pas vocation à reposer sur un humain qui répond mécaniquement « go » ou « no-go » à chaque opération. Le système doit pouvoir comprendre les risques, appliquer ses règles d'autorisation et prendre en charge les opérations ordinaires de façon autonome.

Les opérations dépassant ses autorisations ou présentant des conséquences critiques doivent néanmoins respecter les limites de sécurité établies.

La coopération doit permettre **d'obtenir une décision mieux informée, sans transformer chaque action en procédure manuelle**.

### Des compétences minimales, spécialisées et évolutives

Une compétence n'a pas nécessairement besoin d'être une IA.

Elle peut prendre la forme d'une expression régulière, d'un programme Rust, d'une base de données, d'un analyseur ou d'un petit modèle neuronal.

**Exemple :**

Une IA NLP spécialisée dans le langage peut identifier des expressions insultantes et proposer de les intégrer à un dictionnaire.

Un détecteur symbolique peut ensuite reconnaître ces expressions sans mobiliser de modèle neuronal.

Lorsque la détection nécessite une interprétation, le système peut déléguer à une IA NLP. Celle-ci peut à son tour demander une analyse plus approfondie.

Les propositions d'évolution des mécanismes symboliques doivent pouvoir être vérifiées avant de devenir des règles durables.

L'objectif est de permettre à **de nombreuses compétences extrêmement légères** de coopérer. Certaines IA spécialisées pourraient ne nécessiter qu'environ 1 Go de VRAM, selon leur modèle, leur quantification et leur contexte ; d'autres compétences pourraient fonctionner sans GPU.

Le système n'a pas besoin de conserver simultanément tous ses modèles en mémoire.

**Une compétence peut être très limitée individuellement tout en apportant une contribution importante à l'intelligence collective.**

## 6. Le Miroir Cognitif

BCAIP doit persister et analyser l'historique pérenne pour **comprendre Rémi mieux que Rémi lui-même**.

Cette ambition ne signifie pas que le système possède toujours une meilleure interprétation de l'utilisateur. Elle signifie qu'il doit pouvoir identifier des relations, des contradictions et des causes que l'utilisateur — ou l'IA plongée dans la conversation immédiate — n'a pas nécessairement remarquées.

Le miroir cognitif ne se limite pas à retenir ce que l'utilisateur a dit.

Il cherche à comprendre **pourquoi il l'a dit, dans quel contexte, sur quelles hypothèses et avec quelle intention**.

### La logique derrière les décisions

Rémi peut affirmer une chose dans un projet TypeScript, puis sembler affirmer son contraire dans un projet Rust.

Une mémoire superficielle pourrait conclure à une contradiction.

Le miroir cognitif doit pouvoir retrouver que les deux décisions ont été prises dans des contextes techniques différents.

Il doit également envisager qu'une décision antérieure ait reposé sur une hypothèse incorrecte.

L'objectif n'est pas de mémoriser une règle isolée comme « Rémi préfère A », mais de conserver les conditions qui ont conduit à cette préférence.

Une réflexion exploratoire, une hypothèse, une préférence, une décision validée et une règle durable ne doivent pas être confondues.

**La dernière affirmation de l'utilisateur n'est pas automatiquement la plus pertinente. La plus ancienne n'est pas automatiquement la plus vraie non plus.**

### Comprendre l'utilisateur pour corriger l'IA

Le miroir cognitif doit également aider les IA à comprendre leurs propres erreurs d'interprétation.

Une réaction de colère, par exemple, peut être un signal indiquant que quelque chose s'est mal passé dans l'échange.

Elle ne doit pas être interprétée automatiquement comme une caractéristique de l'utilisateur, ni comme la preuve que celui-ci a tort ou raison.

**Exemple :**

Rémi répond : « Vas te faire foutre. »

Un mécanisme symbolique peut reconnaître l'expression et déclencher une analyse du contexte.

Le système consulte alors les échanges précédents, les observations comportementales pertinentes et les intentions exprimées.

Une IA NLP peut examiner la demande initiale et la réponse de l'assistant. Une autre IA peut rechercher des contradictions.

Le diagnostic pourrait être :

« L'utilisateur demandait X. L'assistant a répondu Y, puis a affirmé l'inverse de ce qu'il disait précédemment. L'utilisateur semble réagir à cette incompréhension. Il faut réévaluer la réponse initiale. »

L'IA principale reçoit alors les informations nécessaires à sa correction, sans devoir reconstituer toute l'histoire conversationnelle.

### Une mémoire contextuelle, explicable et révisable

Le système doit pouvoir distinguer :

- Les observations : ce qui a effectivement été dit ou fait.
- Les interprétations : ce que le système pense avoir compris.
- Les justifications : pourquoi il retient une interprétation.
- Les incertitudes : ce qu'il ne peut pas établir avec confiance.

Les analyses comportementales peuvent évoluer. Une interprétation ancienne peut être corrigée lorsque de nouvelles informations apparaissent.

Le miroir cognitif peut notamment repérer la fatigue apparente, la dispersion, les changements de direction ou les contradictions, sans transformer ces observations en diagnostics ou en vérités définitives.

Son rôle n'est pas de flatter Rémi, de lui imposer un comportement ou de décider à sa place.

**Son rôle est de permettre au système de mieux travailler avec lui, de le contredire lorsqu'il se trompe et de reconnaître lorsque c'est l'IA elle-même qui a mal compris.**

Ce fonctionnement n'est pas réservé au développement logiciel. Il doit pouvoir s'appliquer à l'ensemble des domaines dans lesquels l'utilisateur interagit avec BCAIP.

---

## 7. Une Intelligence Collective plutôt qu'un Cerveau Omniscient

BCAIP n'a pas pour objectif de concentrer toute l'intelligence dans un unique modèle gigantesque.

Un composant peut seulement reconnaître une expression. Un autre peut uniquement effectuer des calculs. Un troisième peut interpréter une intention. Un quatrième peut analyser une contradiction.

Aucun n'est obligé de connaître le fonctionnement interne des autres.

L'ensemble peut coopérer à travers des intentions, des délégations et des résultats adaptés au besoin réel.

Le système doit pouvoir choisir une opération symbolique lorsqu'elle suffit, mobiliser une compétence neuronale lorsqu'une interprétation est nécessaire et faire intervenir d'autres compétences lorsque l'incertitude ou le risque le justifient.

Il peut également réévaluer le résultat d'une coopération et reconnaître qu'une conclusion reste incertaine.

**L'intelligence de BCAIP se trouve autant dans les relations entre ses composants que dans leurs capacités individuelles.**

---

## Principe fondateur

**L'intention dirige. Le symbolique exécute ce qu'il sait faire. Le neuronal interprète ce qui demande de l'intelligence adaptative. La mémoire apporte le contexte. La coopération permet de dépasser les limites individuelles. Le doute permet de corriger les erreurs.**

BCAIP doit être conçu comme un système capable d'apprendre à mieux comprendre les intentions, à mieux répartir les compétences et à mieux travailler avec son utilisateur, sans supposer qu'un seul cerveau puisse tout savoir ni qu'une seule interprétation puisse toujours être correcte.
