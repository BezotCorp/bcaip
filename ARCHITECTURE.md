# BCAIP — Architecture

Statut : architecture initiale — décisions établies et questions ouvertes

Ce document définit les premières frontières architecturales de BCAIP.

Il complète la vision originelle sans la remplacer. Il distingue les décisions retenues des mécanismes techniques qui restent à concevoir.

## 1. Principes fondamentaux

BCAIP est un système composé de plusieurs éléments indépendants, capables de communiquer et de coopérer sans dépendre d'une interface graphique particulière.

Les principes retenus sont les suivants :

- Les composants possèdent des responsabilités distinctes.
- Les communications entre processus passent par le hub.
- Le hub comprend, autorise, transforme et coordonne les échanges.
- Chaque composant reste responsable de ses fonctionnalités et des données qu'il émet.
- Les composants ne doivent pas connaître les détails internes des autres composants pour communiquer.
- Le hub possède une connaissance plus large des échanges que les autres composants.
- Les formats d'échange sont définis dans des bibliothèques spécialisées, réutilisées par les composants concernés.
- Les données communes ne dépendent pas de l'interface utilisée.
- L'architecture permet d'introduire des mécanismes intelligents lorsque des situations complexes le justifient, sans les imposer systématiquement.
- Le découpage des bibliothèques suit leurs responsabilités réelles, sans chercher artificiellement à réduire ou multiplier leur nombre.

## 2. Organisation générale

Le dépôt distingue trois catégories principales :

- `products/` : les composants applicatifs du produit BCAIP.
- `services/` : les services indépendants assurant les fonctions d'infrastructure.
- `libraries/` : les bibliothèques spécialisées, notamment celles qui définissent les formats d'échange.

```text
bcaip/
├── docs/
│   ├── vision.md
│   └── architecture.md
├── products/
│   ├── app_engine/
│   ├── app_ui/
│   └── vscode_extension/
├── services/
│   ├── hub/
│   └── storage/
└── libraries/
    ├── app_engine_exchange_format/
    ├── app_ui_exchange_format/
    ├── vscode_extension_exchange_format/
    └── storage_exchange_format/
```

Cette organisation établit des frontières explicites sans prétendre définir l'ensemble des futurs composants ou bibliothèques.

## 3. Services

### 3.1. Hub

Le hub est le centre de communication et d'échange de BCAIP.

Il constitue le point de passage des communications interprocessus et décide si une communication est autorisée ou refusée.

Il ne se limite pas à transmettre les données reçues.

Ses responsabilités comprennent notamment :

- Identifier les interlocuteurs impliqués dans les échanges.
- Comprendre la signification des données et des demandes qu'il traite.
- Autoriser ou refuser une communication selon les règles applicables.
- Identifier les composants et capacités nécessaires à une demande.
- Construire, transformer, compléter ou adapter les données échangées.
- Coordonner plusieurs opérations et leurs dépendances.
- Conserver les informations nécessaires pour associer les demandes, les réponses et leurs destinataires.
- Exploiter les résultats intermédiaires pour déterminer les opérations suivantes.
- Organiser les échanges en tenant compte des besoins de cohérence, de ressources ou de transaction.

Une demande reçue peut aboutir à plusieurs échanges et opérations avant de produire une réponse ou une nouvelle demande.

Inversement, plusieurs opérations peuvent devoir se terminer avant que le hub ne sollicite un composant comme Storage.

Le hub connaît les formats d'échange nécessaires pour comprendre chacun de ses interlocuteurs. Cette connaissance ne lui impose pas de connaître leurs implémentations internes.

Le hub peut disposer de mécanismes intelligents lorsque les situations rencontrées nécessitent des décisions complexes. Cette possibilité fait partie de l'architecture, mais ne constitue pas une obligation d'implémenter immédiatement une intelligence neuronale.

### 3.2. Storage

Storage est le serveur de stockage indépendant de BCAIP.

Il assure la persistance et l'accès aux données durables relevant de sa responsabilité.

Il communique avec le hub selon ses propres formats d'échange.

Une demande adressée à Storage n'a pas nécessairement la même représentation ni le même contenu que la demande initialement reçue par le hub.

Le hub peut notamment préparer une demande de stockage à partir de plusieurs résultats intermédiaires.

Storage reste responsable de l'exécution de ses opérations et des garanties qu'il fournit.

Les technologies de persistance, les modèles de données et les garanties transactionnelles restent à définir.

## 4. Produits

### 4.1. App Engine

App Engine constitue le moteur applicatif de BCAIP.

Il porte les fonctionnalités liées aux intentions, à la coopération entre compétences et aux mécanismes neuronaux ou symboliques prévus dans la vision du produit.

Il fonctionne indépendamment des interfaces graphiques.

Il communique avec le hub selon les formats d'échange qui les concernent.

Il n'a pas besoin de connaître les formats de Storage ou des autres composants pour solliciter leurs capacités à travers le hub.

Le détail de son organisation interne reste à définir.

### 4.2. App UI

App UI constitue l'interface de l'application autonome.

Elle permet d'utiliser BCAIP indépendamment de VS Code et sans être limitée par les capacités d'affichage d'une extension.

Elle communique avec le hub dans les deux sens.

Elle peut transmettre des demandes et des informations, recevoir des résultats et répondre à des sollicitations.

La technologie graphique reste à déterminer.

### 4.3. VS Code Extension

VS Code Extension constitue l'intégration de BCAIP dans VS Code.

Elle peut présenter une interface adaptée à l'éditeur, transmettre des données issues de son environnement et exposer les capacités que VS Code met à sa disposition.

Elle communique avec le hub dans les deux sens.

Ses contraintes techniques ne doivent pas être imposées à App UI ou App Engine.

Son mode d'exécution doit respecter l'environnement des extensions VS Code.

## 5. Bibliothèques

Les bibliothèques regroupées dans `libraries/` possèdent chacune une responsabilité identifiée.

Elles ne sont pas limitées aux communications : d'autres bibliothèques spécialisées pourront être créées lorsque leurs responsabilités le justifieront.

### 5.1. Bibliothèques de formats d'échange

Les bibliothèques `*_exchange_format` définissent les représentations communes nécessaires aux communications entre un composant et le hub.

Elles peuvent notamment contenir :

- Les structures représentant les demandes.
- Les structures représentant les réponses.
- Les événements et erreurs échangés.
- Les types de données nécessaires à ces échanges.
- Les définitions nécessaires à leur sérialisation et désérialisation.

Elles ne contiennent pas la logique interne des composants, le routage des communications ou leur orchestration.

Chaque bibliothèque correspond à une frontière de communication identifiée.

| Bibliothèque                       | Utilisateurs concernés   |
| ---------------------------------- | ------------------------ |
| `app_engine_exchange_format`       | App Engine et Hub        |
| `app_ui_exchange_format`           | App UI et Hub            |
| `vscode_extension_exchange_format` | VS Code Extension et Hub |
| `storage_exchange_format`          | Storage et Hub           |

Le nom d'une bibliothèque indique le composant auquel son format d'échange est associé. Il ne signifie pas que ce composant en est l'unique utilisateur.

### 5.2. Indépendance des formats

Chaque composant connaît les définitions nécessaires à ses échanges avec le hub.

Le hub connaît les définitions nécessaires à ses échanges avec tous les composants concernés.

Ainsi, App Engine n'a pas besoin de dépendre de `storage_exchange_format`, et Storage n'a pas besoin de dépendre de `app_engine_exchange_format`.

Les deux bibliothèques peuvent définir des structures entièrement différentes.

Le hub peut désérialiser une demande reçue selon un format, en comprendre le contenu, effectuer les traitements nécessaires et construire une nouvelle demande selon un autre format.

Cette séparation évite d'imposer un protocole de données universel à tous les composants.

### 5.3. Responsabilité et réutilisation

Une bibliothèque doit correspondre à une responsabilité cohérente.

Un type comme `Token`, `Session` ou `Message` ne justifie pas automatiquement la création d'une bibliothèque.

Inversement, plusieurs responsabilités indépendantes ne doivent pas être regroupées artificiellement dans une bibliothèque universelle.

Lorsqu'une capacité ou une représentation doit être partagée au-delà d'une frontière d'échange, son emplacement et ses dépendances doivent être étudiés selon son rôle réel.

Le nombre de bibliothèques n'est pas un objectif en soi.

## 6. Communications interprocessus

Le hub constitue le point central des échanges.

```text
          App UI          VS Code Extension
             |                    |
             |                    |
             +---------+----------+
                       |
                       |
                      Hub
                     /   \
                    /     \
                   /       \
             App Engine   Storage
```

Toutes les relations représentées sont bidirectionnelles.

Le schéma représente les frontières de communication. Il ne fixe pas les protocoles de transport, les connexions physiques ni les séquences d'exécution.

### 6.1. Compréhension des données

Le hub doit comprendre les données qu'il reçoit et celles qu'il émet, selon les définitions des échanges concernés.

Il peut ainsi interpréter une demande, appliquer les règles d'autorisation, en déterminer les suites et produire les messages appropriés.

Cette compréhension repose notamment sur les types et les représentations définis dans les bibliothèques de formats d'échange.

Les composants concernés réutilisent ces définitions plutôt que de recréer chacun leur propre version des mêmes structures.

### 6.2. Transformation des demandes

Les échanges ne constituent pas nécessairement une transmission directe d'un message d'origine vers un destinataire final.

Le hub peut :

- Modifier la représentation des données.
- Compléter une demande avec des informations supplémentaires.
- Produire plusieurs demandes à partir d'une seule.
- Regrouper plusieurs résultats dans une nouvelle demande.
- Attendre certaines conditions avant d'engager un échange.
- Adapter sa coordination selon les résultats reçus.

Par exemple, une demande provenant d'App Engine peut conduire le hub à solliciter plusieurs opérations avant de construire une demande destinée à Storage.

Le format utilisé avec App Engine reste indépendant de celui utilisé avec Storage.

### 6.3. Suivi des échanges

Le hub doit pouvoir relier les réponses reçues aux demandes et opérations dont elles dépendent.

Il doit notamment pouvoir déterminer :

- À quelle demande correspond une réponse.
- Quel composant a initié un échange.
- Quels composants doivent recevoir des informations en retour.
- Quelles opérations sont encore en cours ou attendues.
- Quelles conditions sont nécessaires pour poursuivre une coordination.

Les mécanismes précis permettant ce suivi restent à définir.

Une enveloppe de communication peut participer à ce fonctionnement, mais elle ne remplace pas les définitions des données échangées.

### 6.4. Responsabilité de l'émetteur

Chaque composant reste responsable du contenu des données qu'il émet, dans le cadre des formats d'échange et des autorisations applicables.

Le hub est lui-même responsable des données qu'il construit et transmet.

Il n'est pas obligé de restituer intégralement ou à l'identique les messages reçus.

Le destinataire interprète les données selon le format d'échange qui le concerne.

## 7. Intelligence et coordination

L'architecture doit permettre aux composants de disposer de mécanismes intelligents adaptés à leurs responsabilités.

Le hub peut notamment rencontrer des situations nécessitant une analyse ou une coordination complexe.

Cela ne signifie pas que chaque communication nécessite une intelligence neuronale ou un raisonnement élaboré.

Une opération simple peut être traitée de manière déterministe.

Des situations plus complexes peuvent justifier des mécanismes symboliques, neuronaux ou une combinaison de plusieurs approches.

L'intelligence éventuellement utilisée par le hub reste distincte du fonctionnement applicatif d'App Engine.

Les mécanismes précis et leur organisation ne sont pas encore définis.

## 8. Sécurité et autorisations

Le hub décide si les communications interprocessus qui passent par lui sont autorisées.

Cette responsabilité doit être distinguée des autorisations nécessaires pour exécuter certaines opérations.

Une communication autorisée ne donne pas automatiquement accès à toutes les capacités du destinataire.

De même, une intention exprimée par une IA ne constitue pas une autorisation d'accéder aux ressources du système.

L'architecture distingue donc :

1. L'expression d'une intention ou d'une demande.
2. L'autorisation de communication.
3. L'autorisation d'une opération.
4. L'exécution effective de cette opération.

Les mécanismes d'identification, d'authentification, d'isolation et de contrôle des capacités restent à définir.

## 9. Données et interfaces

Les données durables communes à BCAIP restent indépendantes des interfaces utilisées.

Une conversation, une tâche ou une information persistée ne doit pas appartenir exclusivement à App UI ou à VS Code Extension.

Les interfaces peuvent présenter différemment les mêmes informations.

Elles peuvent également apporter des données ou des capacités propres à leur environnement.

Le partage des données ne signifie pas que tous les composants doivent automatiquement recevoir toutes les informations disponibles.

Les échanges restent soumis aux besoins fonctionnels et aux autorisations applicables.

## 10. Indépendance et exécution

Les composants doivent conserver des frontières fonctionnelles claires.

Hub, Storage et App Engine sont conçus comme des processus indépendants qui communiquent au travers du hub.

App UI constitue une application autonome.

VS Code Extension reste soumise au modèle d'intégration et d'exécution de VS Code.

L'indépendance architecturale ne signifie donc pas nécessairement que tous les composants utilisent le même modèle d'exécution.

Les modalités de lancement, d'arrêt, de connexion, de déconnexion et de reprise restent à préciser.

## 11. Décisions restant ouvertes

Les éléments suivants ne sont pas encore tranchés :

- Les structures précises définies dans chaque bibliothèque `*_exchange_format`.
- Les formats de sérialisation et les protocoles de transport.
- Les éventuels formats d'échange propres au hub.
- Les mécanismes d'identification et d'authentification.
- Les règles d'autorisation et leur application effective.
- La découverte et la représentation des capacités des composants.
- Les mécanismes de suivi et de corrélation des échanges.
- Les règles de transformation et de coordination des demandes.
- Les mécanismes éventuels de décision intelligente du hub.
- Les garanties transactionnelles et les responsabilités associées.
- L'organisation interne d'App Engine.
- Les mécanismes de persistance de Storage.
- Les technologies propres à chaque composant.
- Les modalités d'exécution, de déploiement et de reprise.
- Les bibliothèques supplémentaires qui pourraient être justifiées par des responsabilités indépendantes.

Ces questions devront être étudiées progressivement, sans présenter comme acquises des décisions qui n'ont pas encore été prises.

## 12. Principe architectural directeur

BCAIP repose sur des composants aux responsabilités distinctes, organisés entre produits, services et bibliothèques.

Le hub centralise les communications interprocessus, en contrôle les autorisations, comprend les données échangées et peut coordonner ou transformer les opérations.

Storage constitue le serveur de stockage indépendant. App Engine porte le fonctionnement applicatif. App UI et VS Code Extension fournissent des interfaces et des capacités adaptées à leurs environnements.

Les bibliothèques de formats d'échange définissent les représentations partagées entre chaque composant et le hub.

Le hub connaît les différents formats nécessaires à son fonctionnement, sans imposer aux autres composants de connaître ceux de leurs interlocuteurs.

Cette organisation permet de faire évoluer les composants indépendamment, de conserver des responsabilités claires et de développer des mécanismes de coordination plus complexes lorsque cela devient nécessaire.
