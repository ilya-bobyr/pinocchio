# Support for strings in `AccountContent` types

Need to think about the string handling.  I would imagine there are several
cases how on-chain programs can handle strings:
 - Strings with pre-allocated storage.  String length is encoded before the
   string, followed by the storage for up to the maximum string length.

   This is simpler, but I'm not sure if this is efficient, considering UTF-8 can
   use up to 4 bytes per character.  The pre-allocated storage is then, probably
   in bytes, not characters.

   One big advantage is that the string update does not require shifts of the
   rest of the struct data.

   Two disadvantages:
     - Unnecessary allocations permanently increase storage usage.  As the
       developers have to use a pessimistic choice, extra storage allocations
       can be substantial.

     - Even with a pessimistic choice of the maximum size, there is still a
       limit that could be inconvenient for the users.  In particular, non-Latin
       languages can easily use 2 bytes, and Asian languages use 3 bytes per
       character in UTF-8.

 - Strings that use the exact number of bytes necessary for storage.  This could
   be much more efficient, compared to strings with pre-allocated size.  But it
   would be much trickier to support, in particular, if the account data is not
   going through a complete serialization/deserialization cycle.  Growing or
   shrinking such a string would require shifting of all of the subsequent
   bytes.

   There are probably some interesting ways to optimize this.  For example, the
   framework could store the new string in a dynamically allocated array, and
   only update the account bytes at the end of the program execution.  This
   would be mostly as efficient as an in-place update for a single change, and
   more efficient if the string is updated multiple times, or if multiple string
   fields are updated.
