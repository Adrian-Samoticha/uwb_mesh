/// A line of output from the virtual console.
///
/// This struct represents a line of output from the virtual console.
/// It contains the line of text and a boolean indicating whether the line
/// was input or output.
#[derive(Clone, Debug)]
pub struct OutputLine {
    /// The line of text.
    pub line: String,

    /// Whether the line was input or output.
    ///
    /// true if the line was input, false if it was output.
    pub is_input: bool,
}

impl OutputLine {
    /// Construct a new OutputLine.
    ///
    /// # Arguments
    ///
    /// * `line` - The line of text.
    /// * `is_input` - Whether the line was input or output.
    pub fn new(line: String, is_input: bool) -> Self {
        OutputLine { line, is_input }
    }
}

impl Default for OutputLine {
    fn default() -> Self {
        OutputLine {
            line: String::new(),
            is_input: false,
        }
    }
}

/// A console that can be interacted with programmatically.
///
/// This can be used to simulate a console interaction in the simulator.
#[derive(Clone)]
pub struct VirtualConsole {
    /// The output of the console.
    ///
    /// The last element of the deque is the current output line.
    output: std::collections::VecDeque<OutputLine>,

    /// The input buffer.
    ///
    /// This is the input that the user has typed so far.
    input_buffer: String,

    /// The maximum number of lines of output to keep.
    ///
    /// When the output size exceeds this number, the oldest lines are discarded.
    max_size: usize,
}

impl VirtualConsole {
    /// Create a new VirtualConsole with default settings.
    pub fn new() -> Self {
        let mut output = std::collections::VecDeque::new();
        output.push_back(OutputLine::default());

        VirtualConsole {
            output,
            input_buffer: String::new(),
            max_size: 128,
        }
    }

    /// Start a new line in the output.
    fn start_new_line(&mut self, is_input: bool) {
        self.output
            .push_back(OutputLine::new(String::new(), is_input));

        if self.output.len() > self.max_size {
            self.output.pop_front();
        }
    }

    /// Write a character to the output of the console.
    ///
    /// # Arguments
    ///
    /// * `character` - The character to write.
    /// * `is_input` - Whether the character is an input character.
    pub fn write_char_to_output(&mut self, character: char, is_input: bool) {
        if character == '\0' {
            return;
        }

        if character == '\n' {
            self.start_new_line(is_input);
            return;
        }

        if let Some(last) = self.output.back() {
            if last.is_input != is_input {
                self.start_new_line(is_input);
            }
        }

        if let Some(last) = self.output.back_mut() {
            last.line.push(character);
        }
    }

    /// Write a character to the input buffer.
    ///
    /// # Arguments
    ///
    /// * `character` - The character to write.
    pub fn write_char_to_input_buffer(&mut self, character: char) {
        self.input_buffer.push(character);
    }

    /// Get a reference to the output of the console.
    ///
    /// # Returns
    ///
    /// A reference to the output deque.
    pub fn get_output(&self) -> &std::collections::VecDeque<OutputLine> {
        &self.output
    }

    /// Get a mutable reference to the output of the console.
    pub fn get_output_mut(&mut self) -> &mut std::collections::VecDeque<OutputLine> {
        &mut self.output
    }

    /// Get the input buffer.
    ///
    /// # Returns
    ///
    /// A clone of the input buffer string.
    pub fn get_input_buffer(&self) -> String {
        self.input_buffer.clone()
    }

    /// Clear the input buffer.
    pub fn clear_input_buffer(&mut self) {
        self.input_buffer.clear();
    }

    /// Read and remove the first character from the input buffer.
    ///
    /// # Returns
    ///
    /// The first character from the input buffer, or None if the buffer is empty.
    pub fn read_first_char_from_input_buffer(&mut self) -> Option<char> {
        if self.input_buffer.is_empty() {
            return None;
        }
        let result = self.input_buffer.chars().next()?;
        self.input_buffer.remove(0);
        Some(result)
    }

    /// Check if the input buffer is empty.
    ///
    /// # Returns
    ///
    /// True if the input buffer is empty, false otherwise.
    pub fn is_input_buffer_empty(&self) -> bool {
        self.input_buffer.is_empty()
    }

    /// Get the size of the input buffer.
    ///
    /// # Returns
    ///
    /// The number of characters in the input buffer.
    pub fn get_input_buffer_size(&self) -> usize {
        self.input_buffer.len()
    }

    /// Set the maximum number of lines of output to keep.
    ///
    /// # Arguments
    ///
    /// * `new_max_size` - The new maximum number of lines of output to keep.
    pub fn set_max_size(&mut self, new_max_size: usize) {
        self.max_size = new_max_size;
    }
}

impl Default for VirtualConsole {
    fn default() -> Self {
        Self::new()
    }
}
