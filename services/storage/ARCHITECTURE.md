# BCAIP — Storage : vision et architecture

## 1. Vision fondamentale

Storage est le service intelligent de persistance de BCAIP.

Il reçoit des demandes exprimant une intention et possède la liberté de déterminer comment les satisfaire.

Le demandeur n'a pas à connaître le moteur de persistance, l'organisation physique des données, les stratégies d'indexation ou les optimisations internes.

**Storage est responsable du résultat, pas simplement de l'exécution d'une commande sur une base de données.**

Ses décisions doivent être autonomes, explicables, mesurables et améliorables.

## 2. Séparation des responsabilités

Trois responsabilités fondamentales doivent rester distinctes.

- **Décision** : déterminer quelle stratégie utiliser à partir des contraintes, capacités et observations disponibles.
- **Exécution** : confier les opérations aux mécanismes déterministes des moteurs spécialisés.
- **Évaluation** : mesurer les résultats et déterminer si les décisions étaient pertinentes.

Un moteur de règles symbolique constitue la première forme d'intelligence.

Une intelligence artificielle interne pourra ultérieurement proposer des hypothèses ou des stratégies. Elle ne remplacera pas les garanties déterministes d'intégrité et de persistance.

## 3. Indépendance des moteurs

Storage n'est pas un service SQLite.

SQLite constitue son premier moteur réel, mais son architecture doit permettre l'intégration de moteurs relationnels, graphes, documentaires ou spécialisés.

Chaque moteur possède ses capacités, limitations, contraintes, stratégies transactionnelles et caractéristiques de performance.

Storage exploite directement les mécanismes natifs des moteurs. Il ne réimplémente pas inutilement les fonctionnalités qu'ils fournissent déjà.

**La modularité concerne également les stratégies de concurrence : aucun verrouillage ou modèle transactionnel propre à SQLite ne doit être imposé universellement.**

## 4. Intelligence décisionnelle

Storage doit pouvoir :

- Sélectionner une stratégie adaptée à une demande.
- Observer une dégradation ou une amélioration.
- Identifier des possibilités d'optimisation.
- Recommander un autre moteur.
- Expérimenter une alternative.
- Décider de conserver le moteur actuel.
- Déclencher une migration lorsque les conditions sont réunies.
- Réévaluer une décision précédente.

Il peut également produire une recommandation inexécutable, par exemple lorsqu'un moteur potentiellement meilleur n'est pas disponible.

Une recommandation n'implique jamais automatiquement une migration.

**L'absence de changement est une décision valide.**

## 5. Mesures, scores et vérité

L'intelligence doit s'appuyer sur des observations réelles.

Exemples de mesures :

- Latence et distribution des temps de réponse.
- Débit et fréquence des opérations.
- Consommation mémoire.
- Utilisation du stockage.
- Coûts des transactions et de la concurrence.
- Taux d'erreur et conflits.
- Coûts d'une migration ou d'une optimisation.

Une mesure brute doit conserver son unité, son contexte, sa méthode de collecte et ses conditions d'observation.

Une analyse ou un score est une interprétation calculée à partir de ces observations.

**Storage doit toujours distinguer les faits mesurés, les hypothèses, les prédictions et les conclusions.**

Un score ne doit pas représenter une confiance fictive. Sa méthode de calcul doit être identifiable et reproductible.

Une stratégie ne peut être déclarée meilleure que dans les conditions effectivement évaluées.

## 6. Expérimentation et amélioration

Lorsqu'une stratégie semble préférable, Storage doit pouvoir la confronter à l'existant.

Une expérimentation doit permettre de :

1. Définir ce qui est comparé.
2. Collecter des mesures représentatives.
3. Vérifier la comparabilité des conditions.
4. Évaluer les gains et les régressions.
5. Inclure les coûts opérationnels et de migration.
6. Conserver ou rejeter la proposition.
7. Enregistrer les résultats pour les décisions futures.

Les expérimentations ne doivent pas compromettre les données déjà validées.

Une décision incorrecte constitue une information exploitable pour améliorer le moteur de règles.

**L'objectif n'est pas de ne jamais se tromper. L'objectif est de pouvoir découvrir, expliquer et corriger ses erreurs.**

## 7. Mémoire décisionnelle durable

Storage doit conserver son historique d'observation et de décision.

Cette mémoire comprend :

- Les observations et leurs conditions de mesure.
- Les analyses réalisées.
- Les hypothèses et propositions.
- Les règles et versions des règles utilisées.
- Les décisions prises et leurs motivations.
- Les expérimentations effectuées.
- Les résultats mesurés.
- Les régressions et retours arrière.
- Les changements effectivement appliqués.

Les données brutes disponibles doivent rester distinguables des analyses dérivées.

La conservation doit être adaptée au volume : agrégation, rétention et limitation des coûts de télémétrie.

La mémoire décisionnelle est distincte, sur le plan logique, des données confiées à Storage.

## 8. Localisation et migration

Storage doit savoir où résident les données dont il est responsable.

Une donnée déjà enregistrée ne doit pas devenir inaccessible parce qu'une nouvelle stratégie a été sélectionnée.

Les migrations entre moteurs sont autorisées et font partie de l'autonomie de Storage.

Une migration doit préserver les garanties applicables aux données concernées et permettre une transition contrôlée.

Storage doit pouvoir comparer les performances avant et après migration, puis réexaminer sa décision.

La réversibilité doit être prévue lorsqu'elle est techniquement possible ; elle ne doit jamais être supposée gratuitement.

## 9. Garanties non négociables

La liberté décisionnelle ne dispense pas Storage de garantir :

- L'intégrité des données validées.
- La cohérence des opérations et des révisions.
- L'atomicité des opérations qui l'exigent.
- Le respect des autorisations.
- La maîtrise des ressources.
- Des résultats et des erreurs explicites.
- Des comportements définis lors des interruptions.
- La traçabilité des décisions importantes.

**Une stratégie peut échouer. Une hypothèse peut être fausse. Cela ne doit pas entraîner silencieusement une perte ou une corruption des données.**

## 10. Implémentation actuelle et trajectoire

### Base existante

- Service Rust autonome.
- Contrat d'échange `storage_exchange_format`.
- Interface `StorageEngine`.
- Moteur `SqliteEngine`.
- Opérations CRUD avec révisions.
- Cache interne.
- Communication WebSocket.

Ces éléments constituent une base, pas la réalisation complète de la vision.

### Architecture cible

- Catalogue des moteurs et de leurs capacités.
- Moteur de règles décisionnelles.
- Observation des performances.
- Scores fondés sur des mesures.
- Journal durable des analyses et décisions.
- Expérimentation contrôlée.
- Optimisation et migrations autonomes.
- Évolution possible vers une IA interne.

Une capacité future ne doit jamais être présentée comme déjà implémentée.

## 11. Principe directeur

**Storage observe, analyse, décide, exécute, mesure et apprend de ses résultats.**

L'intelligence conserve la liberté de proposer, d'expérimenter et de changer de stratégie.

Les moteurs spécialisés fournissent leurs garanties techniques.

Les observations permettent de distinguer les décisions pertinentes des décisions erronées.

L'historique permet à Storage de ne pas répéter aveuglément ses erreurs.

Cette vision doit guider les évolutions du service sans imposer prématurément une implémentation particulière.
