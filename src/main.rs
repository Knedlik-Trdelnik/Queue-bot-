use rand::seq::{IndexedRandom, SliceRandom};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use log::info;
use teloxide::types::FileId;
use teloxide::types::InputFile;
use teloxide::{prelude::*, utils::command::BotCommands};
use teloxide::dispatching::UpdateHandler;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use toml::Table;

#[derive(Serialize, Deserialize)]
struct User {
    chat_id: ChatId,
    name: String,
    username: String,
    swap_pos: usize,
}

#[derive(Serialize, Deserialize)]
struct Admins {
    admins: Vec<User>,
}
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", parse_with = "split")]
enum Command {
    Start,
    AddMe,
    DeleteMe,
    CreateQueue,
    ShowQueue,
    Info,
    Help,
    Swap(usize),
    Ban(String),
    Unban(String),
    Del(usize),
}

static STUDENTS: LazyLock<RwLock<HashMap<ChatId, User>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static ADMINS: LazyLock<RwLock<HashMap<ChatId, User>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
//TODO:  tokio::sync::RwLock drop(guard)
static QUEUE: LazyLock<RwLock<Vec<ChatId>>> = LazyLock::new(|| RwLock::new(Vec::new()));
//TODO: избавиться от множественных клонирований. В queue хватит &str
static BANNED: LazyLock<RwLock<HashMap<ChatId, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
//Забаненные пользователи

static LIMIT: LazyLock<RwLock<usize>> = LazyLock::new(|| RwLock::new(0));
//Лимит людей в очереди. По умолчанию - 50

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Запускаем бота...");
    //$env:TELOXIDE_TOKEN = "TOKEN"
    //$env:RUST_LOG="info"

    parse_and_init().await;
    let bot = Bot::from_env();
    /**/
    Command::repl(bot, action).await;


    /*
        teloxide::repl(bot, |bot: Bot, msg: Message| async move {
            if let Some(text) = msg.text() {
                bot.send_message(msg.chat.id, text).await?;

            }
            log::info!("{:#?}", msg);
            Ok(())
        })
        .await;
    */
}

async fn parse_and_init() {
    log::info!("Начинаем читать файл конфигурации...");
    let config_name = "config.toml";

    match fs::read_to_string(config_name).await {
        Ok(config) => {
            log::info!("{}", config);
            let mut adm = ADMINS.write().await;

            match toml::from_str::<Admins>(&config) {
                Ok(arr) => {
                    for user in arr.admins {
                        adm.insert(user.chat_id, user);
                    }
                }
                Err(err) => {
                    log::warn!("Не удалось прочитать админов - {}", err);
                }
            }

            let mut lim = LIMIT.write().await;
            let values = config.parse::<Table>().unwrap();

            *lim = values["limit"].as_integer().unwrap_or(50) as usize;
        }

        Err(err) => {
            log::warn!(
                "Не удалось прочитать админов и параметры конфигурации - {}",
                err
            );
        }
    }
}

async fn action(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    {
        let name_for_logger: String = msg.from.clone().unwrap().first_name;
        log::info!(
            "Поступило сообщение от {}. Username:@{}. chatId: {}. Команда: {}",
            name_for_logger,
            msg.from
                .as_ref()
                .unwrap()
                .username
                .as_ref()
                .unwrap()
                .as_str(),
            msg.chat.id,
            msg.text().unwrap()
        );
    }
    {
        let prisoners = BANNED.read().await;
        if prisoners.contains_key(&msg.chat.id) {
            bot.send_message(msg.chat.id, "Упс...а ты забанен").await?;
            return Ok(());
        }
    }
    match cmd {
        Command::Help | Command::Start => {
            bot.send_message(msg.chat.id, "Приветики, это бот для организации очереди учебной группы P3*17.\n\
            Он работает рандомизированным способом: первичный вид очереди определяется рандомом. Далее студни могут обменяться местами(я этого не делал), если хотят подойти в начале/конце практики.\n\
            Доступные команды:\n/addme - добавить вас в список студней\n\
            /deleteme - удалить вас из списка студней\n\
            /createqueue - [только для админов] создать общую очередь для всех ( затирает прошлую )\n\
            /showqueue - показать общую очередь для всех\n\
            /info - показать зарегистрированных пользователей ( в будущем метрику хз )\n\
            /swap [число] - предложить свапнуться со студнем на месте [чиcло]\n\
            /ban - [только для админов] [юзернейм] без @ - забанить зарегистрированного))))\n\
            /unban - [только для админов] [юзернем] без @))))\n\
            /del [число] - выкинуть и удалить человека (у которога в очереди [число] место) из зарегестрированных пользователей\n\
            /help - ну...блин)").await?;
            let sticker_id = FileId(
                "CAACAgIAAxkBAAIG12opX911iwkw7Xaqk3FCqak_OdosAALBaAAC956BScJug_m8nC63OwQ"
                    .to_string(),
            );

            bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
                .await?;
        }
        Command::AddMe => {
            add_me(bot, msg).await?;
        }
        Command::DeleteMe => {
            delete_me(bot, msg).await?;
        }
        Command::CreateQueue => {
            create_queue(bot, msg).await?;
        }
        Command::ShowQueue => {
            show_queue(bot, msg).await?;
        }
        Command::Info => {
            info(bot, msg).await?;
        }
        Command::Swap(position) => {
            swap(bot, msg, position).await?;
        }
        Command::Ban(user_name) => {
            ban(bot, msg, user_name).await?;
        }
        Command::Unban(user_name) => unban(bot, msg, user_name).await?,
        Command::Del(position) => {
            del(bot, msg, position - 1).await?;
        }
    }

    Ok(())
}

async fn add_me(bot: Bot, msg: Message) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "Начинаю добавлять...")
        .await?;

    let user_cont = {
        let map = STUDENTS.read().await;
        map.contains_key(&msg.chat.id)
    };
    if !user_cont {
        let mut map = STUDENTS.write().await;

        {
            let lim = LIMIT.read().await;
            if map.len() >= *lim {
                bot.send_message(
                    msg.chat.id,
                    "Лимит зарегестрированных пользователей достигнут...Пора уже кого-то удалять",
                )
                .await?;
                return Ok(());
            }
        }

        let user = msg.from.as_ref().unwrap();
        let first = user.first_name.clone();
        let last = user.last_name.clone().unwrap_or_default().clone();
        let username = format!("{} {}", first, last);
        let student = User {
            chat_id: msg.chat.id,
            name: username.clone(),
            username: msg
                .from
                .as_ref()
                .unwrap()
                .username
                .as_ref()
                .unwrap()
                .clone(),
            swap_pos: usize::MAX,
        };
        map.insert(msg.chat.id, student);
        let mut q = QUEUE.write().await;
        q.push(msg.chat.id);
    } else {
        bot.send_message(
            msg.chat.id,
            "Погоди, ты уже зарегистрирован. Зачем тебе все это.........???",
        )
        .await?;
        return Ok(());
    }
    bot.send_message(msg.chat.id, "Done (добавлен в конец очереди)")
        .await?;

    Ok(())
}

async fn delete_me(bot: Bot, msg: Message) -> ResponseResult<()> {
    let (user_cont, user_fl_id) = {
        let map = STUDENTS.read().await;
        let is_user_cont = map.contains_key(&msg.chat.id);
        let cht_id: ChatId = msg.chat.id;
        //(map.contains_key(&msg.chat.id), map[&msg.chat.id].clone())
        (is_user_cont, cht_id)
    };

    if user_cont {
        let mut map = STUDENTS.write().await;
        let mut q = QUEUE.write().await;

        let index = {
            let mut some_inx: usize = 0; //МММ как умом вот я умный ммм умом да очень умно ммм
            for id in q.iter() {
                if user_fl_id.eq(id) {
                    break;
                }
                some_inx += 1;
            }
            some_inx
        };
        q.remove(index);

        map.remove(&msg.chat.id);
    }
    bot.send_message(
        msg.chat.id,
        "Done (Удален из текущей очереди иииии...зарегистрированных пльзвтлй)",
    )
    .await?;
    Ok(())
}

async fn info(bot: Bot, msg: Message) -> ResponseResult<()> {
    let a: String = {
        let map = ADMINS.read().await;
        let mut res: String = String::new();
        let mut cnt: u32 = 1;
        res.push_str("Админы с правом перемешивать очередь\n");
        for user in map.values() {
            res.push_str(format!("№{} ", cnt).as_str());
            res.push_str(user.name.as_str());
            res.push_str(" || @");
            res.push_str(user.username.as_str());
            res.push_str("\n");
            cnt += 1;
        }
        let map = STUDENTS.read().await;
        res.push_str("\nСписок зарегистрированных пльзвтлй\n");
        cnt = 1;
        for user in map.values() {
            res.push_str(format!("№{} ", cnt).as_str());
            res.push_str(user.name.as_str());
            res.push_str(" || @");
            res.push_str(user.username.as_str());
            res.push_str("\n");
            cnt += 1;
        }

        res.push_str("\nСписок забаненных пльзвтлй\n");
        let banned = BANNED.read().await;
        cnt = 1;
        for user in banned.values() {
            res.push_str(format!("№{} ", cnt).as_str());
            res.push_str(" || @");
            res.push_str(user.as_str());
            res.push_str("\n");
            cnt += 1;
        }
        res
    };
    if a.is_empty() {
        bot.send_message(msg.chat.id, "Нико не зарегестрирован")
            .await?;
    } else {
        bot.send_message(msg.chat.id, a).await?;
    }

    let sticker_id = FileId(
        "CAACAgIAAxkBAAIGy2opXkmzZkrHZEIeDWxjKc7bRgh5AAKWaQACen2ASdp4Chu6WD7tOwQ".to_string(),
    );

    bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
        .await?;
    Ok(())
}
/*
id: FileId(
                                "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
                            ),

 */
async fn create_queue(bot: Bot, msg: Message) -> ResponseResult<()> {
    if !is_user_admin(&msg.chat.id).await {
        bot.send_message(msg.chat.id, "Эй, ты не админ...фу...")
            .await?;
        let sticker_id = FileId(String::from(
            "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
        ));
        bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
            .await?;
        return Ok(());
    };

    {
        let mut q = QUEUE.write().await;
        q.clear();
        let map = STUDENTS.read().await;

        for id in map.keys() {
            q.push(*id);
        }
        let mut rng = rand::rng();
        q.shuffle(&mut rng);
    }
    let v: Vec<(ChatId, String)> = {
        let map = STUDENTS.read().await;
        let mut res = Vec::new();
        for (id, user) in map.iter() {
            res.push((id.clone(), user.name.clone()));
        }
        res
    };
    let user = msg.from.unwrap();
    let first = user.first_name;
    let last = user.last_name.unwrap_or_default();
    let victim = format!("{} {}", first, last);

    for (id, name) in &v {
        match bot
            .send_message(
                *id,
                format!("{}, очередь была перемешана студнем {}", name, victim),
            )
            .await
        {
            Err(err) => {
                log::warn!(
                    "Не удалось отправить уведомление для {} (ID: {}): {:?}",
                    name,
                    id,
                    err
                );
            }
            Ok(_) => {}
        };
    }

    bot.send_message(msg.chat.id, "Перемешана").await?;
    Ok(())
}
async fn show_queue(bot: Bot, msg: Message) -> ResponseResult<()> {
    let a: String = {
        let vec = QUEUE.read().await;
        let map = STUDENTS.read().await;
        let mut res: String = String::new();
        if vec.is_empty() {
            res = "Очередь пуста".to_string();
        } else {
            let mut cnt: u32 = 1;
            for id in vec.iter() {
                res.push_str(format!("[{}] --> {}\n", cnt, map[id].name).as_str());
                cnt += 1;
            }
        }
        res
    };
    let file_id;
    if a == "Очередь пуста" {
        file_id = FileId::from("CAACAgIAAxkBAAEGFdFqehzcvUVj99Wl2AoxAsB30tczTgAChaIAAkuC2Et-s1rh-1N0Uj0E");
    }
    else{
        let chat_id = msg.chat.id;
        let vec = QUEUE.read().await;
        let first = ["CAACAgIAAxkBAAEGFcNqehzOhBTRjs3zsNbWDFPd-BmOrwACeqUAAkIk0UvP3AQuIv8ONT0E",
        "CAACAgIAAxkBAAEGFc1qehzauTiaWS9IveHl0R43jiLfgAACNqgAAvlZ0UuH-x8UAgN1Ij0E"];
        let second = ["CAACAgIAAxkBAAEGFcdqehzU2eQfGA-KcrTDFvovWTbfBwACragAAmTH0EsGpKQWVYIcwD0E",
        "CAACAgIAAxkBAAEGFdVqeh0-1MvSf9yS5jZS6AzZyVYargACQ6sAAtJ-0EuyBrITYtte0z0E"];
        let third = ["CAACAgIAAxkBAAEGFclqehzXEbC6s8SzrstD_R5XTcec3gACh6MAAvs20EsTG43RZ7vg9j0E",
        "CAACAgIAAxkBAAEGFcVqehzRV5Wb8gppv5d4uAowTHyjqwACVq0AArHA0Utbn-koKCNyqT0E"];
        let fourth = ["CAACAgIAAxkBAAEGFctqehzZsHzytrBna1K_MH_2nqyi2gACtqQAAhV_0Eu5vKwXWOV3dj0E"];
        let fifth = ["CAACAgIAAxkBAAEGFc9qehzbNvZJIDLu9Yjng9ZhmgZD4gACZ58AAiMx0Ev-O-9kxCWDsz0E"];
        let selected_list: &[&str] = match vec.iter().position(|id| *id == chat_id) {
            Some(0) => &first,   // Первое место в очереди
            Some(1) => &second,
            Some(2) => &third,
            Some(3) => &fourth,
            Some(4) => &fifth,
            _ => &["CAACAgEAAxkBAAEGGHtqeuqDTgVo-ijTNGyCvkqkTRHjeAACfQQAAg-CwUfkFNcioKNKQD0E",
            "CAACAgIAAxkBAAEGGH9qeurBW5WA27UqFjvYnqy06nynrwACfqQAAh8V2EvTU2nxJQnIVj0E"]
        };
        file_id = FileId::from(selected_list
            .choose(&mut rand::rng())
            .unwrap()
            .to_string()
        );
    }
    bot.send_message(msg.chat.id, a).await?;
    bot.send_sticker(msg.chat.id, InputFile::file_id(file_id)).await?;
    Ok(())

    // let sticker_id = FileId(
    //     "CAACAgIAAxkBAAIHx2opid3IJDQu2k8Mas6T8St7a4TJAALjZAACN0N4SKETlfOjuuUZOwQ".to_string(),
    // );


}

//поступает на вход @name
async fn find_user_by_name(un: &String) -> Result<ChatId, String> {
    let map = STUDENTS.read().await;
    for user in map.values() {
        if user.username.eq(un) {
            return Ok(user.chat_id);
        }
    }
    Err(format!("Пользователь с именем {} не найден", un))
}

async fn find_banned_by_name(un: &String) -> Result<ChatId, String> {
    let map = BANNED.read().await;
    for (key, value) in &*map {
        if un.eq(value) {
            return Ok(*key);
        }
    }
    Err(format!("Пользователь с именем {} не найден", un))
}

async fn ban(bot: Bot, msg: Message, user_name: String) -> ResponseResult<()> {
    if !is_user_admin(&msg.chat.id).await {
        bot.send_message(msg.chat.id, "Эй, ты не админ...фу...")
            .await?;
        let sticker_id = FileId(String::from(
            "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
        ));
        bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
            .await?;
        return Ok(());
    };
    match find_user_by_name(&user_name).await {
        Ok(id) => {
            let map = ADMINS.read().await;
            if map.contains_key(&id) {
                bot.send_message(msg.chat.id, "Админов нельзя банить....")
                    .await?;
                let sticker_id = FileId(String::from(
                    "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
                ));
                bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
                    .await?;
                return Ok(());
            }
            {
                let mut prisoners = BANNED.write().await;
                let students = STUDENTS.read().await;
                prisoners.insert(id, students.get(&id).unwrap().username.clone());
            }
            delete_user(id).await;
            bot.send_message(msg.chat.id, "Забанен!").await?;
            return Ok(());
        }
        Err(err) => {
            bot.send_message(msg.chat.id, err).await?;
            return Ok(());
        }
    }
}

async fn unban(bot: Bot, msg: Message, user_name: String) -> ResponseResult<()> {
    if !is_user_admin(&msg.chat.id).await {
        bot.send_message(msg.chat.id, "Эй, ты не админ...фу...")
            .await?;
        let sticker_id = FileId(String::from(
            "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
        ));
        bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
            .await?;
        return Ok(());
    };

    match find_banned_by_name(&user_name).await {
        Ok(id) => {
            let mut prisoners = BANNED.write().await;
            prisoners.remove(&id);
            bot.send_message(msg.chat.id, "Разбанен(а)...!").await?;
            Ok(())
        }
        Err(err) => {
            bot.send_message(msg.chat.id, "Он(а) не был(а) забанен(а)...")
                .await?;
            Ok(())
        }
    }
}

async fn delete_user(id: ChatId) {
    let mut map = STUDENTS.write().await;
    let mut q = QUEUE.write().await;

    let index = {
        let mut some_inx: usize = 0; //МММ как умом вот я умный ммм умом да очень умно ммм
        for u_id in q.iter() {
            if id.eq(u_id) {
                break;
            }
            some_inx += 1;
        }
        some_inx
    };
    q.remove(index);
    map.remove(&id);
}

async fn swap(bot: Bot, msg: Message, position: usize) -> ResponseResult<()> {
    //т.к. очередь для пользователя с 1, а у нас все с 0
    let pos: usize = position - 1;
    let index_of_sender = {
        let q = QUEUE.read().await;
        let mut some_inx: usize = 0;
        for u in q.iter() {
            if msg.chat.id.eq(u) {
                break;
            }
            some_inx += 1;
        }

        if some_inx == q.len() {
            return Ok(());
        }
        some_inx
    };

    let mut is_swapped = false;
    let victim_id;

    {
        let mut queue = QUEUE.write().await;

        if pos >= queue.len() {
            bot.send_message(
                msg.chat.id,
                format!(
                    "Индекс слишком большой... В очереди всего {} студней",
                    queue.len()
                ),
            )
            .await?;
            return Ok(());
        }

        victim_id = queue[pos];
        let sender_id = msg.chat.id;

        let mut map = STUDENTS.write().await;
        map.get_mut(&sender_id).unwrap().swap_pos = pos;

        if map.get(&victim_id).unwrap().swap_pos == index_of_sender {
            queue.swap(pos, index_of_sender);
            map.get_mut(&sender_id).unwrap().swap_pos = 0;
            map.get_mut(&victim_id).unwrap().swap_pos = 0;
            is_swapped = true;
        }
    }
    if is_swapped {
        bot.send_message(msg.chat.id, "Свап выполнен").await?;
        bot.send_message(
            victim_id,
            format!("Свап с {} подтвержден", index_of_sender + 1),
        )
        .await?;
    } else {
        bot.send_message(
            victim_id,
            format!(
                "Тебе предложил свап c позицией {}.\nДля подтверждения отправь [/swap {}]",
                index_of_sender + 1,
                index_of_sender + 1
            ),
        )
        .await?;
        bot.send_message(msg.chat.id, "Запрос отправлен... Ждем ответа.")
            .await?;
    }
    Ok(())
}

async fn is_user_admin(id: &ChatId) -> bool {
    let map = ADMINS.read().await;
    if !map.contains_key(id) {
        return false;
    }
    true
}

async fn del(bot: Bot, msg: Message, index: usize) -> ResponseResult<()> {
    if !is_user_admin(&msg.chat.id).await {
        bot.send_message(msg.chat.id, "Эй, ты не админ...фу...")
            .await?;
        let sticker_id = FileId(String::from(
            "CAACAgIAAxkBAAIHJmopa6re4q_lDaO9HvW5nLL4MbzHAAKTeQACnfhwSG3etcvVolCMOwQ",
        ));
        bot.send_sticker(msg.chat.id, InputFile::file_id(sticker_id))
            .await?;
        return Ok(());
    };
    let mut map = STUDENTS.write().await;
    let mut q = QUEUE.write().await;
    if index < q.len() {
        map.remove(&q[index]);
        q.remove(index);
        bot.send_message(msg.chat.id, "Отправлен в пекло!").await?;
    } else {
        bot.send_message(msg.chat.id, "Алоооо, почему пишем несуществующие индексы?")
            .await?;
    }
    Ok(())
}
