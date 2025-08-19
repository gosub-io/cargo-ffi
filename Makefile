CRATE_NAME := gosub-engine
LIB_BASE := gosub_engine

TARGET_DIR := target/release
LIB_NAME := lib$(LIB_BASE).so

HEADER := gosub.h
CBINDGEN := cbindgen
C_SRC := c_example/main.c
C_BIN := c_example/gosub_c_example

all: $(TARGET_DIR)/$(LIB_NAME) $(HEADER)

$(TARGET_DIR)/$(LIB_NAME):
	cargo build --release

$(HEADER):
	$(CBINDGEN) --crate $(CRATE_NAME) --output $(HEADER)

run_rust:
	cargo run --example gtk_example

run_c:
	gcc $(C_SRC) -o $(C_BIN) -L$(TARGET_DIR) -l$(LIB_BASE) -I. -ldl
	LD_LIBRARY_PATH=$(TARGET_DIR) ./$(C_BIN)

run_python: $(TARGET_DIR)/$(LIB_NAME) $(HEADER)
	cd python_example && python3 main.py

clean:
	cargo clean
	rm -f $(HEADER)
	rm -f $(C_BIN)

.PHONY: all c_example clean
