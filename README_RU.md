# git-remote-dev

**Git Remote Helper для протокола `dev://` сети NETFORY.**
Даёт привычные команды `git clone / push / pull dev://user/repo` - без
переучивания, с нативной поддержкой в любой IDE (VS Code, WebStorm,
GitKraken), потому что проектом управляет ваш стандартный бинарник `git`.

## Требования

- `git` ≥ 2.30 в `PATH` (мост использует `git bundle` под капотом);
- запущенный **клиент NETFORY** с разблокированным кошельком -
  мост общается с ним через loopback-IPC (порт/токен берёт из
  discovery-файла, см. ниже);
- Rust toolchain - только для сборки.

## Сборка и установка

```bash
cargo build --release
```

Положите бинарник в каталог из `PATH` **строго под именем** `git-remote-dev`
(git ищет хелпер по схеме URL: `dev://` → `git-remote-dev`):

```bash
# Linux / macOS
install -m 0755 target/release/git-remote-dev ~/.local/bin/git-remote-dev
# проверьте, что ~/.local/bin в PATH:  echo $PATH

# Windows (PowerShell)
copy target\release\git-remote-dev.exe C:\Users\<you>\bin\git-remote-dev.exe
# каталог C:\Users\<you>\bin должен быть в %PATH%
```

Проверка установки:

```bash
git clone dev://<ваш-username>/<ваш-репозиторий>
```

## Настройка

Настройки не нужны. Мост сам находит работающий клиент через discovery-файл,
который клиент пишет при старте:

| ОС | Путь |
|---|---|
| Linux / macOS | `~/.config/smartnet/devhub-ipc.json` |
| Windows | `%APPDATA%\smartnet\devhub-ipc.json` |

Содержимое: `{"port": <loopback-порт>, "token": "<случайный токен>"}`.
Токен защищает IPC от чужих локальных процессов; сервер слушает **только**
`127.0.0.1`.

## Использование

```bash
git clone dev://technolog/smart-swarm     # клонировать из P2P-сети
cd smart-swarm
git add . && git commit -m "feat: …"      # обычная локальная работа
git push origin main                      # опубликовать: подпись + DHT-анонс
```

- `git push` в ещё не существующее имя репозитория **автосоздаёт** его
  в вашем аккаунте (как на GitHub).
- Каждый push увеличивает подписанный счётчик `seq` вашего индекса -
  анти-откат: сеть никогда не примет более старую версию.
- Клонирование чужих репозиториев работает через swarm-загрузку
  bundle'ов по подписанному `bundle_hash` (сидеры находятся через
  Mainline DHT + UDP identity-биконы).

## Типичные ошибки

| Сообщение | Причина / решение |
|---|---|
| `клиент NETFORY не запущен (нет discovery-файла…)` | Откройте приложение NETFORY и повторите. |
| `Кошелёк заблокирован - разблокируйте PIN-кодом в клиенте` | Разблокируйте кошелёк в клиенте (push подписывается вашим ключом). |
| `push разрешён только в свои репозитории` | URL указывает на чужой username. |
| `сидеры репозитория сейчас офлайн` | Ни один пир с bundle'ом не в сети - попробуйте позже. |
| `git не найден в PATH` | Установите git / добавьте в PATH. |

Подробное описание архитектуры: `docs/09-DevHub-Git-Remote-Helper.md`.
