use inquire::Password;

pub fn edit_wallet() {
    let name = Password::new("Encryption key:").prompt();
}
