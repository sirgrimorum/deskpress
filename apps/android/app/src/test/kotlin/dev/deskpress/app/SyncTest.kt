package dev.deskpress.app

import java.io.ByteArrayInputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URI
import org.junit.Assert.assertEquals
import org.junit.Test

class SyncTest {
    @Test
    fun onlyAnHttpsAddressIsFetchedAndAFailureNeverRepeatsIt() {
        assertEquals(Reply(0, "not an address"), get("http://a.org/f?k=secret"))
        assertEquals(Reply(0, "not an address"), get("https://a b/f?k=secret"))
        assertEquals(Reply(0, "not an address"), get("f?k=secret"))
    }

    @Test
    fun aHostThatDoesNotAnswerIsNoAnswer() {
        // Port 1 on this machine refuses at once, with no network involved.
        assertEquals(Reply(0, "no answer"), get("https://127.0.0.1:1/f"))
    }

    @Test
    fun aReplyThatNeverEndsIsCutOffPastOneMegabyte() {
        val endless = object : InputStream() {
            override fun read() = 'a'.code
        }
        assertEquals(Reply(0, "the reply is over 1 MB"), reply(Answer(200, ok = endless)))
        val exact = ByteArrayInputStream(ByteArray(1 shl 20) { 'a'.code.toByte() })
        assertEquals(1 shl 20, reply(Answer(200, ok = exact)).body.length)
    }

    @Test
    fun anErrorKeepsItsStatusWithWhateverBodyCameWithIt() {
        assertEquals(Reply(404, ""), reply(Answer(404)))
        assertEquals(Reply(500, "down"), reply(Answer(500, error = "down".byteInputStream())))
    }

    @Test
    fun aGoodReplyIsItsBody() {
        assertEquals(Reply(200, "{\"t\": 1}"), reply(Answer(200, ok = "{\"t\": 1}".byteInputStream())))
    }

    /** A connection that answers `status` with the given streams, and never touches a network. */
    private class Answer(
        private val status: Int,
        private val ok: InputStream? = null,
        private val error: InputStream? = null,
    ) : HttpURLConnection(URI("https://a.org/").toURL()) {
        override fun getResponseCode() = status
        override fun getInputStream() = ok
        override fun getErrorStream() = error
        override fun connect() {}
        override fun disconnect() {}
        override fun usingProxy() = false
    }
}
