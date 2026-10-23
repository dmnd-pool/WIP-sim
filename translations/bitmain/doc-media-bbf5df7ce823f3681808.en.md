# HashCore proxy groups and failover pools

Source: `doc-media-bbf5df7ce823f3681808`.
Scope: all visible Russian labels in this diagram. The diagram was checked visually.

| Original label | English translation |
| --- | --- |
| Порт | Port |
| Прокси-группа 1 | Proxy group 1 |
| Вес: 1.0 | Weight: 1.0 |
| Прокси-группа 2 | Proxy group 2 |
| Вес: 0.6 | Weight: 0.6 |
| Основной пул | Primary pool |
| Резервный пул | Backup pool |

The port connects to both proxy groups. Each group has a primary pool and a backup pool. The two group weights are 1.0 and 0.6. The diagram does not give a failover time or a retry rule.
