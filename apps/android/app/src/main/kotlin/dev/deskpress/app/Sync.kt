package dev.deskpress.app

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.text.input.PasswordVisualTransformation
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URI
import java.net.URISyntaxException
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** What an address answered: its HTTP status, or 0 when nothing did, with `body` saying why. */
data class Reply(val status: Int, val body: String)

// The most a reply may hold: a forecast is a few kilobytes.
private const val LIMIT = 1 shl 20

/**
 * A GET of `url`. It blocks, so it runs off the main thread. A redirect is not followed: it
 * would reach a host nobody approved. A failure never repeats the address, which may hold a key.
 */
fun get(url: String): Reply =
    try {
        val uri = URI(url)
        if (uri.scheme != "https") throw URISyntaxException(url, "not https")
        val c = uri.toURL().openConnection() as HttpURLConnection
        c.instanceFollowRedirects = false
        c.connectTimeout = 15_000
        c.readTimeout = 15_000
        try {
            reply(c)
        } finally {
            c.disconnect()
        }
    } catch (_: IOException) {
        Reply(0, "no answer")
    } catch (_: URISyntaxException) {
        Reply(0, "not an address")
    } catch (_: IllegalArgumentException) {
        Reply(0, "not an address")
    }

/** What an open connection answered, its body cut off past [LIMIT]. */
fun reply(c: HttpURLConnection): Reply {
    val status = c.responseCode
    val stream = if (status in 200..299) c.inputStream else c.errorStream
    val body = stream?.use { it.upTo(LIMIT + 1) } ?: ByteArray(0)
    return if (body.size > LIMIT) Reply(0, "the reply is over 1 MB") else Reply(status, body.decodeToString())
}

/** At most `limit` bytes of the stream: the cap stops a reply that never ends. */
private fun InputStream.upTo(limit: Int): ByteArray {
    val out = ByteArrayOutputStream()
    val chunk = ByteArray(8192)
    while (out.size() < limit) {
        val n = read(chunk, 0, minOf(chunk.size, limit - out.size()))
        if (n < 0) break
        out.write(chunk, 0, n)
    }
    return out.toByteArray()
}

/**
 * A pack's secrets, like the key a forecast asks for: one file each under `dir`, sealed with a
 * key the Android Keystore holds, so the file alone gives nothing away. Never in the pack. A slot
 * is `pack/host/name`, so a secret is only ever sent to the host it was given for.
 */
class Secrets(private val dir: File) {
    /** The secret, or null when there is none or it can no longer be opened. */
    fun read(slot: String): String? {
        val sealed = File(dir, slot).takeIf { it.exists() }?.readBytes() ?: return null
        if (sealed.size <= IV) return null
        return try {
            val cipher = Cipher.getInstance(AES)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV))
            cipher.doFinal(sealed, IV, sealed.size - IV).decodeToString()
        } catch (_: GeneralSecurityException) {
            null
        }
    }

    /** Seals and keeps the secret. When the Keystore refuses, it is not kept and asked again next time. */
    fun write(slot: String, value: String) {
        try {
            val cipher = Cipher.getInstance(AES)
            cipher.init(Cipher.ENCRYPT_MODE, key())
            val file = File(dir, slot)
            file.parentFile?.mkdirs()
            file.replace(cipher.iv + cipher.doFinal(value.encodeToByteArray()))
        } catch (_: GeneralSecurityException) {
        }
    }

    /** Drops the secret, as when the server turned it down. */
    fun forget(slot: String) {
        File(dir, slot).delete()
    }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val purposes = KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        val spec = KeyGenParameterSpec.Builder(ALIAS, purposes)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE).apply { init(spec) }.generateKey()
    }

    private companion object {
        const val KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "deskpress.secrets"
        const val AES = "AES/GCM/NoPadding"
        const val IV = 12
    }
}

/** Asks once whether the pack may fetch from `hosts`; the answer holds until they change. */
@Composable
fun AllowHosts(hosts: List<String>, answer: (Boolean) -> Unit) {
    AlertDialog(
        onDismissRequest = { answer(false) },
        title = { Text("Fetch from the internet") },
        text = {
            Column {
                Text("This pack keeps some of its data up to date from:")
                for (host in hosts) Text(host)
            }
        },
        confirmButton = { TextButton({ answer(true) }) { Text("Allow") } },
        dismissButton = { TextButton({ answer(false) }) { Text("Not now") } },
    )
}

/** Asks for the secret `name` sent to `host`, which this phone then keeps sealed; null when cancelled. */
@Composable
fun AskSecret(name: String, host: String, answer: (String?) -> Unit) {
    var value by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = { answer(null) },
        title = { Text("A key for $name") },
        text = {
            Column {
                Text("Sent only to $host.")
                OutlinedTextField(value, { value = it }, singleLine = true, visualTransformation = PasswordVisualTransformation())
            }
        },
        confirmButton = { TextButton({ answer(value.trim().ifEmpty { null }) }) { Text("Keep") } },
        dismissButton = { TextButton({ answer(null) }) { Text("Cancel") } },
    )
}
