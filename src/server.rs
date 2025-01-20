use tokio::{net::TcpListener, io::{self, AsyncWriteExt, AsyncBufReadExt}};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GameState {
    board: [[i32; 7]; 6],  // Tabuleiro 6x7
    current_turn: i32, // 1 para jogador 1 (X), -1 para jogador 2 (O)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Move {
    col: usize, // Coluna onde a peça será jogada
}

#[derive(Debug, Clone)]
struct Player {
    symbol: i32, // 1 para "X", -1 para "O"
    address: String,
}

struct GameRoom {
    game_state: GameState,
    game_started: bool,
    players: Vec<Player>,
}

impl GameRoom {
    fn new() -> Self {
        GameRoom {
            game_state: GameState {
                board: [[0; 7]; 6], 
                current_turn: 1, 
            },
            game_started: false,
            players: Vec::new(),
        }
    }


    // Atualiza o estado do jogo com base na jogada
    pub fn update_game_state(&mut self, player_move: Move) -> Result<(), String> {
        if !is_valid_move(&self.game_state, &player_move) {
            return Err("Jogada inválida. Escolha uma coluna vazia.".to_string());
        }

        // Encontra a linha disponível para a jogada
        let mut row = 5; // Começa da última linha
        while row > 0 && self.game_state.board[row][player_move.col] != 0 {
            row -= 1;
        }
        
        // Coloca a peça na linha e coluna corretas
        self.game_state.board[row][player_move.col] = self.game_state.current_turn;

        // Alterna o turno
        self.game_state.current_turn = -self.game_state.current_turn;

        Ok(())
    }

    // Retorna o estado atual do jogo como uma string em formato bonito
    pub fn get_game_state(&self) -> String {
        let mut chars = Vec::new();
        for i in 1..50 {
            chars.push('\n');
        }
        for row in &self.game_state.board {
            for cell in row {
                if *cell == 0 { chars.push('+'); }
                else if *cell == 1 { chars.push('X'); }
                else if *cell == -1 { chars.push('O'); }
            }
            chars.push('\n');
        }
        let s = chars.into_iter().collect();
        s
    }
}

async fn handle_client(mut stream: tokio::net::TcpStream, game_room: Arc<Mutex<GameRoom>>, player_symbol: i32) {
    let (reader, mut writer) = io::split(stream);
    let mut reader = io::BufReader::new(reader);
    let mut buffer = String::new();

    loop {
        // Não é seu turno. Só atualiza pra ver se tem update(fazer um aviso de que não é seu turno)
        let mut game_room_lock = game_room.lock().await;
        let game_state_str = game_room_lock.get_game_state();
        let _ = writer.write_all(game_state_str.as_bytes()).await;
        if game_room_lock.game_state.current_turn != player_symbol {
            drop(game_room_lock);
            continue;
        }

        buffer.clear();

        if reader.read_line(&mut buffer).await.is_err() {
            println!("Erro ao ler mensagem do cliente");
            break;
        }

        let parts: Vec<&str> = buffer.trim().split_whitespace().collect();
        if parts.len() == 1 {
            if let Ok(col) = parts[0].parse::<usize>() {
                let player_move = Move { col };

                match game_room_lock.update_game_state(player_move) {
                    Ok(_) => {
                        let game_state_str = game_room_lock.get_game_state();
                        let _ = writer.write_all(game_state_str.as_bytes()).await;
                        
                        if check_winner(&game_room_lock.game_state, player_symbol) {
                            let msg = "Você venceu!\n";
                            let _ = writer.write_all(msg.as_bytes()).await;
                            break;
                        }
                    }
                    Err(msg) => {
                        let _ = writer.write_all(msg.as_bytes()).await;
                    }
                }

            } else {
                let msg = "Coluna inválida. Use o formato: número da coluna (ex: 1)\n";
                let _ = writer.write_all(msg.as_bytes()).await;
            }
        } else {
            let msg = "Formato de jogada inválido. Use o formato: número da coluna (ex: 1)\n";
            let _ = writer.write_all(msg.as_bytes()).await;
        }
    }
}

// Verificar se uma jogada é válida
fn is_valid_move(game_state: &GameState, player_move: &Move) -> bool {
    player_move.col < 7 && game_state.board[0][player_move.col] == 0
}

// Função para verificar se algum jogador venceu
fn check_winner(game_state: &GameState, player_symbol: i32) -> bool {
    for row in 0..6 {
        for col in 0..4 {
            if game_state.board[row][col] == player_symbol
                && game_state.board[row][col + 1] == player_symbol
                && game_state.board[row][col + 2] == player_symbol
                && game_state.board[row][col + 3] == player_symbol
            {
                return true;
            }
        }
    }

    for col in 0..7 {
        for row in 0..3 {
            if game_state.board[row][col] == player_symbol
                && game_state.board[row + 1][col] == player_symbol
                && game_state.board[row + 2][col] == player_symbol
                && game_state.board[row + 3][col] == player_symbol
            {
                return true;
            }
        }
    }

    for row in 0..3 {
        for col in 0..4 {
            if game_state.board[row][col] == player_symbol
                && game_state.board[row + 1][col + 1] == player_symbol
                && game_state.board[row + 2][col + 2] == player_symbol
                && game_state.board[row + 3][col + 3] == player_symbol
            {
                return true;
            }
        }
    }

    for row in 3..6 {
        for col in 0..4 {
            if game_state.board[row][col] == player_symbol
                && game_state.board[row - 1][col + 1] == player_symbol
                && game_state.board[row - 2][col + 2] == player_symbol
                && game_state.board[row - 3][col + 3] == player_symbol
            {
                return true;
            }
        }
    }

    false
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("Servidor iniciado na porta 8080");

    let game_room = Arc::new(Mutex::new(GameRoom::new()));

    while let Ok((stream, _)) = listener.accept().await {
        let game_room_clone = Arc::clone(&game_room);
        let mut game_room_lock = game_room_clone.lock().await;

        if game_room_lock.players.len() < 2 {
            let player_symbol = if game_room_lock.players.is_empty() {
                game_room_lock.players.push(Player {
                    symbol: 1,
                    address: stream.peer_addr().unwrap().to_string(),
                });
                1
            } else {
                game_room_lock.players.push(Player {
                    symbol: -1,
                    address: stream.peer_addr().unwrap().to_string(),
                });
                -1
            };

            drop(game_room_lock);

            tokio::spawn(async move {
                handle_client(stream, game_room_clone, player_symbol).await;
            });
        } else {
            let (_, mut writer) = io::split(stream);
            let _ = writer.write_all(b"Jogo j\xE1 cheio, aguarde uma nova partida!\n").await;
        }
    }
}
