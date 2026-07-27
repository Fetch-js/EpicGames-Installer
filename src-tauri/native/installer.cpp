#include <algorithm>
#include <cstddef>
#include <cstring>
#include <filesystem>
#include <string>
#include <system_error>

namespace fs = std::filesystem;

namespace {
void write_error(char* buffer, const std::size_t size, const std::string& message) noexcept {
    if (buffer == nullptr || size == 0) {
        return;
    }
    const auto length = std::min(size - 1, message.size());
    std::memcpy(buffer, message.data(), length);
    buffer[length] = '\0';
}
}  // namespace

extern "C" int install_payload(
    const char* source,
    const char* destination,
    char* error_buffer,
    const std::size_t error_buffer_size) noexcept {
    try {
        if (source == nullptr || destination == nullptr || destination[0] == '\0') {
            write_error(error_buffer, error_buffer_size, "Installation paths are invalid");
            return 1;
        }

        const auto source_path = fs::u8path(source);
        const auto destination_path = fs::u8path(destination);
        if (!fs::is_directory(source_path)) {
            write_error(error_buffer, error_buffer_size, "Installation payload is missing");
            return 2;
        }

        std::error_code error;
        fs::create_directories(destination_path, error);
        if (error) {
            write_error(error_buffer, error_buffer_size, "Cannot create destination: " + error.message());
            return 3;
        }

        for (const auto& entry : fs::recursive_directory_iterator(source_path)) {
            const auto relative = fs::relative(entry.path(), source_path);
            const auto target = destination_path / relative;
            if (entry.is_directory()) {
                fs::create_directories(target);
            } else if (entry.is_regular_file()) {
                fs::create_directories(target.parent_path());
                fs::copy_file(entry.path(), target, fs::copy_options::overwrite_existing);
            }
        }

        return 0;
    } catch (const fs::filesystem_error& error) {
        write_error(error_buffer, error_buffer_size, "File operation failed: " + std::string(error.what()));
        return 4;
    } catch (const std::exception& error) {
        write_error(error_buffer, error_buffer_size, error.what());
        return 5;
    } catch (...) {
        write_error(error_buffer, error_buffer_size, "Unknown native installer error");
        return 6;
    }
}
